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
    /// An enum declaration.
    Enum {
        name: String,
        tokens: Vec<Token>,
    },
    /// An impl block.
    Impl {
        target: String,
        methods: Vec<Function>,
    },
    /// A generic raw item (e.g. const, static).
    Raw {
        attrs: Vec<Attribute>,
        tokens: Vec<Token>,
    },
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
    /// An array type (e.g. `[T; N]`).
    Array {
        base: Box<Type>,
        len: String,
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

use crate::lexer::Token;

/// A statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// Local variable binding: `let [mut] name [: ty] [= init];`
    Local {
        name: String,
        ty: Option<Type>,
        init: Option<Vec<Token>>,
        is_mut: bool,
    },
    /// An if-else conditional statement.
    If {
        cond: Vec<Token>,
        then_branch: Block,
        else_branch: Option<Block>,
    },
    /// A nested block of statements.
    Block(Block),
    /// A return statement.
    Return(Option<Vec<Token>>),
    /// A raw sequence of tokens representing any other statement.
    Raw { tokens: Vec<Token>, has_semi: bool },
}
