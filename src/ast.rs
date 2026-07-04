//! Abstract Syntax Tree (AST) definitions for the Crust compiler.
//!
//! Defines the intermediate representations of Crust programs, functions,
//! structs, types, statements, and expressions.

/// A complete Crust program consists of a sequence of top-level items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item in a Crust program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A function declaration.
    Fn(Function),
    /// A struct declaration.
    Struct(Struct),
    /// A use import statement (e.g. `use core::ffi::*;`).
    Use(String),
}

/// An attribute prepended to an item (e.g. `#[repr(C)]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub tokens: String,
}

/// A parameter in a function signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// A function declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_type: Option<Type>,
    pub body: Block,
    pub is_unsafe: bool,
    pub abi: Option<String>,
    pub attrs: Vec<Attribute>,
}

/// A field in a struct declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

/// A struct declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<Field>,
    pub attrs: Vec<Attribute>,
}

/// Representation of types in the AST, supporting C primitives and pointers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A C primitive type (e.g. `int`, `void`, `char`).
    Primitive(PrimitiveType),
    /// A user-defined or standard Rust type (e.g. `MyStruct`, `i32`).
    UserDefined(String),
    /// A raw pointer type (postfix `*` or `const*` in Crust; e.g. `T*` or `T const*`).
    Pointer {
        base: Box<Type>,
        is_const: bool,
    },
    /// A reference type (e.g. `&mut T` or `&T`).
    Reference {
        base: Box<Type>,
        is_mut: bool,
    },
}

/// C and Rust primitive types supported in Crust.
///
/// C types are parsed as `Type::UserDefined` strings and mapped during
/// the transform pass. Only standard Rust primitives use this enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    /// Standard Rust primitive types (e.g. `i32`, `u64`, `f64`, `bool`).
    RustPrimitive(String),
}

/// A block of statements, which may be marked unsafe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub is_unsafe: bool,
}

/// A statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// Local variable binding: `let [mut] name [: ty] [= init];`
    Local {
        name: String,
        ty: Option<Type>,
        init: Option<Expr>,
        is_mut: bool,
    },
    /// An expression without a trailing semicolon.
    Expr(Expr),
    /// An expression with a trailing semicolon.
    Semi(Expr),
}

/// An expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// An identifier reference.
    Ident(String),
    /// An integer literal.
    IntLit(String),
    /// A string literal.
    StrLit(String),
    /// A character literal.
    CharLit(char),
    /// A binary operation (e.g. `a + b`).
    Binary {
        left: Box<Expr>,
        op: String,
        right: Box<Expr>,
    },
    /// A unary operation (e.g. `-a`, `!a`).
    Unary {
        op: String,
        expr: Box<Expr>,
    },
    /// A function call: `name(args...)`.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// Assignment: `target = value`.
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    /// Pointer dereference: `*expr`.
    Deref(Box<Expr>),
    /// Address of expression: `&expr` or `&mut expr`.
    AddrOf {
        expr: Box<Expr>,
        is_mut: bool,
    },
    /// A sub-block of code.
    Block(Block),
    /// An if-else expression.
    If {
        cond: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// A return statement.
    Return(Option<Box<Expr>>),
}
