//! Abstract Syntax Tree definitions for the Carbide transpiler.
//!
//! Only function and struct *signatures* are parsed structurally so that
//! Carbide type-mapping and pointer-flip transformations can be applied
//! precisely to declared types.  Everything else (function bodies, enum
//! bodies, free-standing items) is stored as **verbatim source text** and
//! emitted unchanged after type-name substitution.

/// A complete Carbide program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level item in a Carbide program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A function / procedure declaration.
    Fn(Function),
    /// A struct declaration (fields parsed for type mapping).
    Struct(Struct),
    /// A `use` import.
    Use(String),
    /// An enum declaration.  Body text is stored verbatim.
    Enum {
        name: String,
        /// Raw source text between `{` and `}`.
        body: String,
    },
    /// An `impl` block.  Methods are parsed so `proc`/`fn` transforms apply.
    Impl {
        target: String,
        methods: Vec<Function>,
    },
    /// A `type Name = Type;` alias declaration.  The RHS type is parsed so
    /// C type mapping and pointer flips apply (e.g. C `typedef` of a
    /// function pointer: `type AudioCallback = fn(buffer: void*, frames: uint) -> void;`).
    TypeAlias {
        name: String,
        ty: Type,
    },
    /// Any other top-level item (`const`, `static`, `type`, …).
    /// Stored entirely as raw source text.
    Raw {
        attrs: Vec<Attribute>,
        src: String,
    },
}

/// An attribute attached to an item (e.g. `#[repr(C)]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub tokens: String,
}

/// A function / procedure declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_type: Option<Type>,
    /// Raw source text of the function body (between outer `{` and `}`).
    /// Type-name substitutions are applied to this string by the transform pass.
    pub body_src: String,
    /// True for `proc` (or explicit `unsafe fn`) → emitted as `unsafe fn`,
    /// body executed in an inherently-unsafe context; no inner `unsafe {}` needed.
    pub is_unsafe: bool,
    pub abi: Option<String>,
    pub attrs: Vec<Attribute>,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// A struct field.
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

/// A type expression in a signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// A standard Rust primitive (`i32`, `u8`, `bool`, …).
    Primitive(PrimitiveType),
    /// A user-defined or mapped C FFI type name.
    UserDefined(String),
    /// A raw pointer (`*mut T` / `*const T`).
    Pointer { base: Box<Type>, is_const: bool },
    /// A reference (`&mut T` / `&T`).
    Reference { base: Box<Type>, is_mut: bool },
    /// An array (`[T; N]`).
    Array { base: Box<Type>, len: String },
    /// A C function pointer type: `fn(param: Type, …) -> Ret`.
    ///
    /// Emitted as `Option<unsafe extern "C" fn(…) -> Ret>` because C
    /// callbacks are nullable and FFI-safe via pointer-null optimization.
    FnPointer {
        params: Vec<Param>,
        ret: Option<Box<Type>>,
    },
}

/// Standard Rust primitive type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimitiveType {
    RustPrimitive(String),
}
