use ariadne::{Color, Label, Report, ReportKind, Source};
use rustyline::{
    Config, Context, DefaultEditor, Editor, Helper,
    completion::{Completer, Pair},
    error::ReadlineError,
    highlight::Highlighter,
    hint::{Hinter, HistoryHinter},
    history::DefaultHistory,
    validate::Validator,
};
use std::collections::BTreeSet;
use std::panic::{self, AssertUnwindSafe};
use std::process::exit;

use crate::cli::InputOpts;
use crate::cst::cst_parse;
use crate::interp::except::exception_unwind_str;
use crate::interp::exec::exec_proc;
use crate::interp::heap::{
    InterpClassStore, InterpImmediateHeap, InterpObj, InterpObjRef, InterpVal,
};
use crate::interp::memoize::InterpMemoize;
use crate::interp::serialize::{SerializeOpts, serialize};
use crate::interp::stack::{InterpStack, InterpStackEntry};
use crate::ir::def::IRCfg;
use crate::ir::lower::CSTIRLower;

#[derive(Default)]
pub struct DebugHelper {
    pub commands: Vec<&'static str>,
    pub hinter: HistoryHinter,
}

impl DebugHelper {
    pub fn new() -> Self {
        Self {
            commands: vec![
                "break",
                "continue",
                "exit",
                "heap",
                "heap-dump",
                "help",
                "n",
                "next",
                "nobreak",
                "noprint",
                "params",
                "print",
                "ptr-dump",
                "repeat",
                "stack",
                "vars",
                "vars-dump",
            ],
            hinter: HistoryHinter {},
        }
    }
}

impl Helper for DebugHelper {}
impl Highlighter for DebugHelper {}
impl Validator for DebugHelper {}

impl Hinter for DebugHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Completer for DebugHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];
        let mut parts = prefix.split_whitespace();

        // Completing the first word
        if parts.clone().count() <= 1 {
            let matches = self
                .commands
                .iter()
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();

            return Ok((0, matches));
        }

        // Completing subcommands
        let first = parts.next().unwrap();
        let second_prefix = parts.next().unwrap_or("");

        let subcmds = match first {
            "break" | "nobreak" => vec![
                "block",
                "branch",
                "call",
                "catch",
                "proc",
                "ret",
                "try_end",
                "try_start",
            ],
            "heap" => vec!["is_immed", "immed"],
            "stack" => vec!["dump", "ptr", "len"],
            _ => vec![],
        };

        let matches = subcmds
            .iter()
            .filter(|s| s.starts_with(second_prefix))
            .map(|s| Pair {
                display: s.to_string(),
                replacement: s.to_string(),
            })
            .collect();

        let start = prefix.rfind(' ').map(|i| i + 1).unwrap_or(0);
        Ok((start, matches))
    }
}

pub type DebugEditor = Editor<DebugHelper, DefaultHistory>;

pub struct DebugData {
    pub step: bool,
    pub try_start: bool,
    pub try_end: bool,
    pub catch: bool,
    pub branch: bool,
    pub block: bool,
    pub break_src: bool,
    pub call: bool,
    pub ret: bool,
    pub print: bool,
    pub print_src: bool,
    pub blocks: BTreeSet<usize>,
    pub procs: BTreeSet<String>,
    pub rl: DebugEditor,
    pub n_repeat: usize,
    pub repeat_cmd: String,
    pub code_lhs: usize,
    pub code_rhs: usize,
    pub src: String,
    pub srcname: String,
}

impl DebugData {
    pub fn from_src(src: String, srcname: String) -> Self {
        let config = Config::builder().enable_signals(true).build();
        let mut rl: DebugEditor = Editor::with_config(config).unwrap();
        rl.set_helper(Some(DebugHelper::new()));

        Self {
            step: true,
            try_start: false,
            try_end: false,
            catch: false,
            print: false,
            print_src: false,
            branch: false,
            break_src: false,
            block: false,
            call: false,
            ret: false,
            blocks: BTreeSet::new(),
            procs: BTreeSet::new(),
            rl,
            n_repeat: 0,
            repeat_cmd: String::new(),
            code_lhs: 0,
            code_rhs: 0,
            src,
            srcname,
        }
    }

    pub fn get_src(&self) -> (String, String) {
        (self.src.clone(), self.srcname.clone())
    }

    pub fn set_src(&mut self, src: String, srcname: String) {
        self.src = src;
        self.srcname = srcname;
    }
}

pub fn debug_ctrl(
    vars: &mut [InterpVal],
    params: &InterpVal,
    stack: &mut InterpStack,
    heap: &mut InterpImmediateHeap,
    memo: &mut InterpMemoize,
    cstore: &mut InterpClassStore,
    data: &mut DebugData,
    iopts: &InputOpts,
    rl: &mut DefaultEditor,
) {
    let mut stop = false;
    while !stop {
        let line = if data.n_repeat == 0 {
            match data.rl.readline("(debug) ") {
                Ok(line) => {
                    let _ = data.rl.add_history_entry(line.as_str());
                    line
                }
                Err(ReadlineError::Interrupted) => {
                    exit(1);
                }
                Err(ReadlineError::Eof) => {
                    exit(0);
                }
                Err(err) => {
                    eprintln!("readline error: {err}");
                    exit(1);
                }
            }
        } else {
            data.n_repeat -= 1;
            data.repeat_cmd.clone()
        };
        let mut iter = line.split_whitespace();

        if let Some(cmd) = iter.next() {
            match cmd {
                "break" => {
                    if let Some(param) = iter.next() {
                        match param {
                            "block" => {
                                if let Some(idx) = iter.next()
                                    && let Ok(n) = idx.parse::<usize>()
                                {
                                    data.blocks.insert(n);
                                } else {
                                    data.block = true;
                                }
                            }
                            "branch" => data.branch = true,
                            "call" => data.call = true,
                            "catch" => data.catch = true,
                            "proc" => {
                                if let Some(proc) = iter.next() {
                                    data.procs.insert(proc.to_string());
                                } else {
                                    eprintln!("break proc requires a procedure tag");
                                }
                            }
                            "ret" => data.ret = true,
                            "source" => data.break_src = true,
                            "try_end" => data.try_end = true,
                            "try_start" => data.try_start = true,
                            _ => eprintln!("invalid break parameter"),
                        }
                    } else {
                        eprintln!("break requires a parameter");
                    }
                }
                "continue" => {
                    data.step = false;
                    stop = true;
                }
                "exit" => exit(0),
                "exec" => {
                    let input = iter.collect::<String>();
                    let (src, srcname) = data.get_src();
                    data.set_src(input.clone(), String::from("execute"));

                    if let Err(e) = panic::catch_unwind(AssertUnwindSafe(|| {
                        let new_opts = iopts.exec_opts();
                        let stmt = cst_parse(&input, &new_opts);
                        let stmt_proc = IRCfg::from_stmt(&stmt, &new_opts);
                        let out = exec_proc(
                            stmt_proc,
                            &InterpVal::Undefined,
                            stack,
                            memo,
                            cstore,
                            data,
                            &new_opts,
                            rl,
                        );

                        eprintln!(
                            "{}",
                            serialize(
                                &out,
                                vars,
                                stack,
                                memo,
                                cstore,
                                data,
                                iopts,
                                rl,
                                SerializeOpts::default()
                            )
                        );

                        if let InterpVal::Ref(r) = out {
                            unsafe {
                                r.invalidate();
                            }
                        }
                    })) {
                        eprint!("uncaught unwind: ");
                        exception_unwind_str(e, vars, stack, memo, cstore, data, iopts, rl);
                    }
                    data.set_src(src, srcname);
                }
                "heap" => {
                    if let Some(idx) = iter.next() {
                        if let Ok(addr) = usize::from_str_radix(&idx[2..], 16) {
                            eprintln!(
                                "{}",
                                serialize(
                                    &InterpVal::Ref(InterpObjRef(addr as *mut InterpObj)),
                                    vars,
                                    stack,
                                    memo,
                                    cstore,
                                    data,
                                    iopts,
                                    rl,
                                    SerializeOpts::default()
                                )
                            );
                        } else {
                            match idx {
                                "is_immed" => {
                                    if let Ok(addr) = usize::from_str_radix(&idx[2..], 16) {
                                        eprintln!(
                                            "{}",
                                            heap.refs
                                                .contains(&InterpObjRef(addr as *mut InterpObj))
                                        );
                                    } else {
                                        eprintln!("invalid heap index");
                                    }
                                }
                                "immed" => {
                                    heap.refs.iter().for_each(|i| eprintln!("{:?}", &i.0));
                                }
                                _ => eprintln!("invalid heap index"),
                            }
                        }
                    } else {
                        eprintln!("invalid heap index");
                    }
                }
                "heap-dump" => {
                    if let Some(idx) = iter.next()
                        && let Ok(addr) = usize::from_str_radix(&idx[2..], 16)
                    {
                        eprintln!("{:?}", unsafe { &*(addr as *mut InterpObj) });
                    } else {
                        eprintln!("invalid variable index");
                    }
                }
                "help" => eprintln!(concat!(
                    "break [...]\n",
                    "\tblock (idx)\n",
                    "\tbranch\n",
                    "\tcall\n",
                    "\tproc [proc_id]\n",
                    "\tret\n",
                    "\tsource\n",
                    "\ttry_end\n",
                    "\ttry_start\n",
                    "continue\n",
                    "exec\n",
                    "exit\n",
                    "heap [idx|...]\n",
                    "\tis_immed [idx]\n",
                    "\tlen\n",
                    "heap-idx [idx]\n",
                    "location\n",
                    "n | next\n",
                    "nobreak [...]\n",
                    "\tblock (idx)\n",
                    "\tbranch\n",
                    "\tcall\n",
                    "\tproc [proc_id]\n",
                    "\tret\n",
                    "\ttry_end\n",
                    "\ttry_start\n",
                    "params\n",
                    "print\n",
                    "print-source\n",
                    "ptr-dump [hex]\n",
                    "repeat [n] [cmd]\n",
                    "stack [idx|...]\n",
                    "\tget-idx [name]\n",
                    "\tdump [idx]\n",
                    "\tlen\n",
                    "\tptr\n",
                    "vars [idx]\n",
                    "vars-dump [idx]\n",
                )),
                "n" | "next" => stop = true,
                "nobreak" => {
                    if let Some(param) = iter.next() {
                        match param {
                            "block" => {
                                if let Some(idx) = iter.next()
                                    && let Ok(n) = idx.parse::<usize>()
                                {
                                    data.blocks.remove(&n);
                                } else {
                                    data.block = false;
                                }
                            }
                            "branch" => data.branch = false,
                            "call" => data.call = false,
                            "catch" => data.catch = false,
                            "proc" => {
                                if let Some(proc) = iter.next() {
                                    data.procs.remove(proc);
                                } else {
                                    eprintln!("break proc requires a procedure tag");
                                }
                            }
                            "ret" => data.ret = false,
                            "try_end" => data.try_end = false,
                            "try_start" => data.try_start = false,
                            _ => eprintln!("invalid nobreak parameter"),
                        }
                    } else {
                        eprintln!("nobreak requires a parameter");
                    }
                }
                "noprint" => data.print = false,
                "params" => {
                    eprintln!(
                        "{}",
                        serialize(
                            params,
                            vars,
                            stack,
                            memo,
                            cstore,
                            data,
                            iopts,
                            rl,
                            SerializeOpts::default()
                        )
                    );
                }
                "print" => data.print = true,
                "print-source" => data.print_src = true,
                "ptr-dump" => {
                    if let Some(idx) = iter.next()
                        && let Ok(addr) = usize::from_str_radix(&idx[2..], 16)
                    {
                        let ptr = addr as *const InterpVal;
                        // SAFETY: the debugger can fully inspect the process
                        eprintln!("{:?}", unsafe { &*ptr })
                    } else {
                        eprintln!("invalid ptr-dump parameter");
                    }
                }
                "repeat" => {
                    if let Some(idx) = iter.next()
                        && let Ok(n) = idx.parse::<usize>()
                    {
                        data.n_repeat = n;
                        data.repeat_cmd = iter.collect();
                    } else {
                        eprintln!("invalid repeat parameter");
                    }
                }
                "stack" => {
                    if let Some(idx) = iter.next() {
                        if let Ok(n) = idx.parse::<usize>()
                            && n < stack.frames.len()
                        {
                            let stack_ptr = stack as *mut InterpStack;
                            match &stack.frames[n] {
                                InterpStackEntry::StackFrameBoundary => {
                                    eprintln!("stack frame boundary")
                                }
                                InterpStackEntry::Variable(v) => {
                                    eprintln!(
                                        "{}: {}",
                                        &v.var,
                                        serialize(
                                            &v.val,
                                            vars,
                                            // SAFETY: non-invalidating mutable borrow
                                            unsafe { &mut *stack_ptr },
                                            memo,
                                            cstore,
                                            data,
                                            iopts,
                                            rl,
                                            SerializeOpts::default(),
                                        )
                                    )
                                }
                                InterpStackEntry::Alias(v) => eprintln!(
                                    "{}: {:p} {}",
                                    &v.var,
                                    v.ptr,
                                    serialize(
                                        unsafe { &*v.ptr },
                                        vars,
                                        // SAFETY: non-invalidating mutable borrow
                                        unsafe { &mut *stack_ptr },
                                        memo,
                                        cstore,
                                        data,
                                        iopts,
                                        rl,
                                        SerializeOpts::default(),
                                    )
                                ),
                            }
                        } else {
                            match idx {
                                "dump" => {
                                    if let Some(idx) = iter.next()
                                        && let Ok(n) = idx.parse::<usize>()
                                        && n < stack.frames.len()
                                    {
                                        eprintln!("{:?}", stack.frames[n]);
                                    } else {
                                        eprintln!("invalid stack parameter");
                                    }
                                }
                                "ptr" => eprintln!("{:p}", stack.frames.as_ptr()),
                                "len" => eprintln!("{}", stack.frames.len()),
                                "get-idx" => {
                                    if let Some(name) = iter.next() {
                                        eprintln!("{:?}", stack.get_pos(name));
                                    } else {
                                        eprintln!("missing name");
                                    }
                                }
                                _ => eprintln!("invalid stack parameter"),
                            }
                        }
                    } else {
                        eprintln!("stack requires parameter");
                    }
                }
                "vars" => {
                    if let Some(idx) = iter.next() {
                        if let Ok(n) = idx.parse::<usize>()
                            && n < vars.len()
                        {
                            let vars_ptr = vars as *mut [InterpVal];

                            eprintln!(
                                "{}",
                                serialize(
                                    &vars[n],
                                    // SAFETY: non-invalidating mutable borrow
                                    unsafe { &mut *vars_ptr },
                                    stack,
                                    memo,
                                    cstore,
                                    data,
                                    iopts,
                                    rl,
                                    SerializeOpts::default()
                                )
                            );
                        } else if idx == "match-ref"
                            && let Some(addr) = iter.next()
                        {
                            let matched = vars
                                .iter()
                                .enumerate()
                                .filter_map(|(idx, i)| {
                                    if let InterpVal::Ref(r) = i
                                        && format!("{:?}", &r.0) == addr
                                    {
                                        Some(idx)
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();
                            eprintln!("{:?}", matched);
                        } else {
                            eprintln!("invalid variable index");
                        }
                    } else {
                        eprintln!("invalid variable index");
                    }
                }
                "vars-dump" => {
                    if let Some(idx) = iter.next()
                        && let Ok(n) = idx.parse::<usize>()
                        && n < vars.len()
                    {
                        eprintln!("{:?}", &vars[n]);
                    } else {
                        eprintln!("invalid variable index");
                    }
                }
                "location" => {
                    Report::build(
                        ReportKind::Advice,
                        (
                            &data.srcname,
                            data.code_lhs..data.code_rhs,
                        ),
                    )
                    .with_label(
                        Label::new((
                            &data.srcname,
                            data.code_lhs..data.code_rhs,
                        ))
                        .with_color(Color::Yellow),
                    )
                    .finish()
                    .eprint((&data.srcname, Source::from(&data.src)))
                    .unwrap();
                }
                _ => eprintln!("unrecognized command"),
            }
        } else {
            stop = true;
        }
    }
}
