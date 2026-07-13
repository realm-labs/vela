mod attributes;
mod control;
mod expr;
#[cfg(test)]
mod expr_lambda_tests;
#[cfg(test)]
mod expr_path_tests;
#[cfg(test)]
mod expr_tests;
mod items;
#[cfg(test)]
mod items_tests;
mod literal_semantics;
mod patterns;
mod statements;
mod syntax;

pub use attributes::{
    SyntaxAttribute, SyntaxAttributeArg, SyntaxAttributeArray, SyntaxAttributeMap,
    SyntaxAttributeMapEntry, SyntaxAttributeValue,
};
pub use control::{SyntaxMatchArm, SyntaxMatchArmBody, SyntaxMatchArmList, SyntaxMatchExpr};
pub use expr::{
    SyntaxArgList, SyntaxArgument, SyntaxArrayExpr, SyntaxAssignExpr, SyntaxAwaitExpr,
    SyntaxBinaryExpr, SyntaxCallExpr, SyntaxExpression, SyntaxExpressionKind, SyntaxFieldExpr,
    SyntaxIndexExpr, SyntaxInterpolatedStringPart, SyntaxInterpolation, SyntaxLambdaBody,
    SyntaxLambdaExpr, SyntaxLiteral, SyntaxMapEntry, SyntaxMapExpr, SyntaxParenExpr,
    SyntaxPathExpr, SyntaxRecordExpr, SyntaxRecordExprField, SyntaxRecordExprFieldList,
    SyntaxTryExpr, SyntaxTupleExpr, SyntaxUnaryExpr, SyntaxUnitExpr,
};
pub use items::{
    SyntaxConstItem, SyntaxEnumItem, SyntaxEnumVariant, SyntaxEnumVariantList, SyntaxFunctionItem,
    SyntaxGlobalItem, SyntaxImplItem, SyntaxImplMethod, SyntaxItem, SyntaxParam, SyntaxParamList,
    SyntaxRecordFieldList, SyntaxStructField, SyntaxStructFieldList, SyntaxStructItem,
    SyntaxTraitItem, SyntaxTraitMethod, SyntaxTupleFieldList, SyntaxUseItem, SyntaxUsePath,
};
pub use patterns::{
    SyntaxPattern, SyntaxPatternKind, SyntaxRecordPattern, SyntaxRecordPatternField,
    SyntaxTuplePattern,
};
pub use statements::{
    SyntaxBreakStmt, SyntaxContinueStmt, SyntaxElseBranch, SyntaxExprStmt, SyntaxForStmt,
    SyntaxIfExpr, SyntaxLetStmt, SyntaxReturnStmt, SyntaxStatement, SyntaxStatementKind,
};
pub use syntax::{
    AstChildren, AstNode, SyntaxBlock, SyntaxSourceFile, SyntaxTypeArgList, SyntaxTypeHint,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Visibility {
    Private,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    Bool(bool),
    Integer(IntegerLiteral),
    Float(FloatLiteral),
    Char(char),
    String(String),
    Bytes(Vec<u8>),
}

impl Literal {
    #[must_use]
    pub fn integer(text: impl Into<String>) -> Self {
        Self::Integer(IntegerLiteral::unsuffixed(text))
    }

    #[must_use]
    pub fn float(text: impl Into<String>) -> Self {
        Self::Float(FloatLiteral::unsuffixed(text))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerLiteral {
    pub text: String,
    pub radix: IntRadix,
    pub suffix: Option<IntegerSuffix>,
}

impl IntegerLiteral {
    #[must_use]
    pub fn unsuffixed(text: impl Into<String>) -> Self {
        let text = text.into();
        let radix = IntRadix::from_literal_text(&text);
        Self {
            text,
            radix,
            suffix: None,
        }
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_text_with_suffix(&self) -> String {
        let mut text = self.text.clone();
        if let Some(suffix) = self.suffix {
            text.push_str(suffix.text());
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntRadix {
    Binary,
    Decimal,
    Hex,
}

impl IntRadix {
    #[must_use]
    pub fn from_literal_text(text: &str) -> Self {
        if text.starts_with("0x") || text.starts_with("0X") {
            Self::Hex
        } else if text.starts_with("0b") || text.starts_with("0B") {
            Self::Binary
        } else {
            Self::Decimal
        }
    }

    #[must_use]
    pub const fn base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Decimal => 10,
            Self::Hex => 16,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerSuffix {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntegerSuffix {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FloatLiteral {
    pub text: String,
    pub suffix: Option<FloatSuffix>,
}

impl FloatLiteral {
    #[must_use]
    pub fn unsuffixed(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            suffix: None,
        }
    }

    #[must_use]
    pub fn source_text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_text_with_suffix(&self) -> String {
        let mut text = self.text.clone();
        if let Some(suffix) = self.suffix {
            text.push_str(suffix.text());
        }
        text
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FloatSuffix {
    F32,
    F64,
}

impl FloatSuffix {
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    IdentityEqual,
    IdentityNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Range,
    RangeInclusive,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignOp {
    Set,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}
