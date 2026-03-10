pub mod call;
pub mod stubs;

use strum_macros::{Display, EnumString};

use crate::cli::InputOpts;
use crate::interp::except::{exception_kind_num_get, exception_val_get};
use crate::interp::get::InterpImmedVal;
use crate::interp::heap::{InterpImmediateHeap, InterpObj, InterpVal};

#[derive(Clone, Copy, Display, Debug, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinProc {
    Contains,
    Cartesian,
    Pow,
    /* returns amount of elements for terms
     */
    Amount,
    // term := term_new(name, len, is_tterm);
    TermNew,
    TermKindEq,
    ListNew,
    /*
     * @t_list: ptrs to list invalidated
     * @t_i: consumed
     *
     * _ := list_push(t_list, t_i);
     */
    ListPush,
    /*
     * @t_in: list, set, or string
     *        if list invalidates list ptrs
     *
     * t_out := pop(t_in);
     */
    Pop,
    StackGetOrNew,
    /* @t_out: bool
     *
     * t_out := stack_in_scope("var");
     */
    StackInScope,
    SetRange,
    ListRange,
    SetNew,
    /*
     * @t_i: consumed
     *
     * t_ptr:<ptr, om> = set_insert(t_set, t_i);
     */
    SetInsert,
    /*
     * @t_order: bool
     *  true  -> first
     *  false -> last
     *
     * t_out := set_borrow(t_set, t_order);
     */
    SetBorrow,
    /*
     * @t_order: bool
     *  true  -> first
     *  false -> last
     *
     * t_out := set_take(t_set, t_order);
     */
    SetTake,
    /*
     * ptr := set_get_tag(set, expr);
     */
    SetGetTag,
    // set := set_get_tag_all(set, expr);
    SetGetTagAll,
    Exit,
    /* @name: immediate string
     *
     * o := object_new(name);
     */
    ObjectNew,
    /*
     * @var: string immediate
     *
     * t_ptr := object_get_or_new(t_obj, "var");
     */
    ObjectGetOrNew,
    /*
     * t_ptr<ptr|om> := object_get(t_obj, "var");
     */
    ObjectGet,
    /*
     * t_ptr := object_add(t_obj, "var");
     */
    ObjectAdd,
    /*
     * _ := object_add_image(t_obj, t_image);
     */
    ObjectAddImage,
    // t_iter := object_iter_new(t_obj);
    ObjectIterNew,
    // t_cond := object_iter_next(t_iter, t_key_addr, t_val_ptr_addr);
    ObjectIterNext,
    /*
     * @val: shared
     * @ret_val: shared
     *
     * _ := cache_add(proc, val, ret_val);
     */
    CacheAdd,
    CacheLookup,
    // _ := cache_clear(proc);
    CacheClear,
    /* @cross_frame: bool
     *
     * _ := stack_alias(name, ptr, cross_frame);
     */
    StackAlias,
    // ptr := stack_add(name);
    StackAdd,
    /*
     * @t_l: variable
     * @i: lifetime tied to @t_l
     *
     * i := iter_new(t_l);
     */
    IterNew,
    IterNext,
    // _ := stack_pop(name)
    StackPop,
    StackFrameAdd,
    StackFramePop,
    /* @kind: immediate number or BuiltinVar::ExceptionKind
     * @except: owned value (marked by call as persistent)
     *
     * _ := throw(kind, except);
     * unreachable;
     */
    Throw,
    Rethrow,
    ExceptionThrow,
    ExceptionSet,
    ExceptionReset,
    /*
     * Invalidates the current stack frame and saves it to the heap.
     * The resulting stack image will only contain values from the current frame.
     */
    StackFrameSave,
    /* @name: immediate string
     *
     * _ := class_add(name, static_proc, constructor_proc);
     */
    ClassAdd,
    Copy,
    Invalidate,
    MarkPersist,
    MarkImmed,
    TypeOf,
    /*
     * @flags:
     *  0x01 ANCHORED
     *  0x02 MULTILINE
     *
     * t_regex := regex_compile(pattern, flags)
     *
     * Panics on error
     */
    RegexCompile,
    /*
     * t_assign := regex_match_groups(t_expr, t_regex, t_matched_addr)
     */
    RegexMatchGroups,
    /*
     * t_matched := regex_match(t_expr, t_regex);
     */
    RegexMatch,
    /*
     * @t_len_addr: set to match length
     * @t_pos_addr: set to match start offset
     *
     * t_matched := regex_match_len(t_expr, t_regex, t_len_addr, t_pos_addr);
     */
    RegexMatchLen,
    /*
     * t_assign := regex_match_groups_len(t_expr, t_regex, t_matched_addr, t_len_addr, t_pos_addr);
     */
    RegexMatchGroupsLen,
    /* outdates existing stack ptrs
     *
     * _ := stack_frame_restore(t_image);
     */
    StackFrameRestore,
    /*
     * Create a stack image that stores copies of all values that are reachable
     * from the current stack context.
     */
    StackFrameCopy,
    StackCopy,
    /*
     * @proc_idx: procedure
     * @t_info: ast consumed var or undefined
     * @t_stack: stack image consumed or undefined
     * @cross_frame: bool
     *
     * target := procedure_new(proc_idx, t_info, t_stack, cross_frame);
     */
    ProcedureNew,
    /* @stack: borrowed stack or undefined
     *
     * stack := procedure_stack_get(procedure);
     */
    ProcedureStackGet,
    /* @slice: lifetime bound to input
     *
     * slice := slice(input, lower, upper);
     */
    Slice,
    /*
     * @t_list: ptrs to list invalidated
     *
     * _ := list_resize(t_list, t_len);
     */
    ListResize,
    /*
     * n := ast_node_new("name", ...);
     */
    AstNodeNew,
    /*
     * @t_n: amount of parameters
     *
     * n := ast_node_new_sized("name", t_n);
     */
    AstNodeNewSized,
    // tag<string|om> := ast_tag_get(tterm_tag, tterm_list_len);
    AstTagGet,
    // tag<string|om> := ast_tterm_tag_get(ast_tag);
    AstTTermTagGet,
    // string := serialize(t);
    Serialize,
    /*
     * Prints a string to stderr
     *
     * _ := print_stdout(str);
     */
    PrintStderr,
    /*
     * Prints a string to stdout
     *
     * _ := print_stdout(str);
     */
    PrintStdout,
    /*
     * @prompt: string to be printed before stdin is read
     *
     * _ := read_line_stdin(prompt);
     */
    ReadLineStdin,
    Ln,
    Exp,
    Sqrt,
    Round,
    Floor,
    Ceil,
    Sin,
    Cos,
    Tan,
    SinH,
    CosH,
    TanH,
    Ulp,
    Eval,
    EvalTerm,
    Execute,
    ParseAst,
    ParseAstBlock,
    /*
     * opts: 0x01 O_READ
     *       0x02 O_WRITE
     *       0x04 O_APPEND
     *       0x08 O_CREAT
     *
     * file := open_at(".", "input", opts);
     */
    OpenAt,
    /* @out: <str>
     *
     * out := read_all(file);
     */
    ReadAll,
    /* @out: [<str>]
     *
     * out := read_all_list(file);
     */
    ReadAllList,
    // _ := write(file, str);
    Write,
    /*
     * @file: str
     *
     * _ := delete(file);
     */
    Delete,
    // _ := sleep(msecs);
    Sleep,
    /* @t_ret: f64 in range 0..1
     *
     * t_ret := rnd_float();
     */
    RndFloat,
    /*
     * t_num := str_val(t_str);
     */
    StrVal,
    UnixEpoch,
    ToChar,
    Cmp,
    ParseInt,
    ParseFloat,
    /* t_out[0][<str>] stdout
     * t_out[1][<str>] stderr
     *
     * t_out<list> := cmd(t_str);
     */
    Cmd,
    IsPrime,
    IsProbablePrime,
}

#[derive(Clone, Copy, Debug, Display, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinVar {
    ExceptionVal,
    /* 0 catch lng
     * 1 catch usr
     * 2 backtrack
     */
    ExceptionKind,
    Params,
    LibraryPath,
    SourcePath,
}

impl BuiltinVar {
    pub fn to_immed_val(
        &self,
        proc_params: &InterpVal,
        opts: &InputOpts,
        heap: &mut InterpImmediateHeap,
    ) -> InterpImmedVal {
        match self {
            BuiltinVar::ExceptionVal => {
                InterpImmedVal::from_val(exception_val_get(), heap).confirm()
            }
            BuiltinVar::ExceptionKind => {
                InterpImmedVal::from_val(InterpVal::Double(exception_kind_num_get() as f64), heap)
            }
            BuiltinVar::Params => InterpImmedVal::from_val(proc_params.clone(), heap).confirm(),
            BuiltinVar::LibraryPath => InterpImmedVal::from_val(
                InterpVal::Ref(heap.push_obj(InterpObj::String(opts.lib_path.to_string()))),
                heap,
            ),
            BuiltinVar::SourcePath => InterpImmedVal::from_val(
                InterpVal::Ref(heap.push_obj(InterpObj::String(
                    opts.path.parent().unwrap().to_string_lossy().into_owned(),
                ))),
                heap,
            ),
        }
    }

    pub fn to_val(&self, proc_params: &InterpVal) -> InterpVal {
        match self {
            BuiltinVar::ExceptionVal => exception_val_get(),
            BuiltinVar::ExceptionKind => InterpVal::Double(exception_kind_num_get() as f64),
            BuiltinVar::Params => proc_params.clone(),
            _ => panic!("lib_path, source_path is not allowed in this context"),
        }
    }
}
