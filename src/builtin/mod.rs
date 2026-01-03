use strum_macros::{EnumString, Display};

#[derive(Clone, Copy, Display, Debug, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinProc {
    Contains,
    Cartesian,
    Pow,
    Amount,
    TermNew,
    TermAdd,
    TermKindEq,
    ListNew,
    /*
     * @t_i: consumed
     *
     * _ := list_push(t_list, t_i);
     */
    ListPush,
    StackGetAssert,
    StackGetOrNew,
    SetRange,
    ListRange,
    SetNew,
    /*
     * @t_i: consumed
     *
     * _ := set_insert(t_set, t_i);
     */
    SetInsert,
    /*
     * @t_n: consumed
     *
     * _ := set_extend(t_set, t_n);
     */
    SetExtend,
    /*
     * @t_n: consumed
     *
     * _ := set_extend(t_set, t_n);
     */
    ListExtend,
    Exit,
    ObjectGetAssert,
    CacheAdd,
    StackAlias,
    StackAdd,
    CacheLookup,
    IterNew,
    IterNext,
    StackPop,
    StackFrameAdd,
    StackFramePop,
    Throw,
    StackFrameSave,
    /* @stack: consumed
     *
     * o := object_new(name, stack);
     */
    ObjectNew,
    /* @stack: consumed
     *
     * _ := class_static_set(name, stack);
     */
    ClassStaticSet,
    ClassAdd,
    ListRefSlice,
    RefSlice,
    Copy,
    Invalidate,
    TypeOf,
    TypeAssert,
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
     * t_matched := regex_match_len(t_expr, t_regex, t_len_addr, t_pos_addr);
     */
    RegexMatchLen,
    /*
     * t_assign := regex_match_groups_len(t_expr, t_regex, t_matched_addr, t_len_addr, t_pos_addr);
     */
    RegexMatchGroupsLen,
    StackFrameRestore,
    StackFrameCopy,
    StackCopy,
    /*
     * @t_stack: consumed
     * @t_info: consumed
     *
     * target := closure_new(proc_idx, t_stack, t_info);
     */
    ClosureNew,
    /*
     * @t_info: consumed
     *
     * target := procedure_new(proc_idx, t_info);
     */
    ProcedureNew,
    Slice,
    ListTaggedGet,
    ListResize,
    Assert,
    RegexCompileMultiLine,
    RegexMatchGroupsOffset,
    SetListGet,
    AstNodeNew,
    AstNodeKindEq,
    AstNodeKindStrEq,
    AstAssignEq,
}

#[derive(Clone, Copy, Debug, Display, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum BuiltinVar {
    ExceptionVal,
    ExceptionKind,
    Params,
}
