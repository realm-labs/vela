use rowan::Language;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum SyntaxKind {
    SourceFile,
    Attribute,
    AttributeArg,
    AttributeArray,
    AttributeMap,
    AttributeMapEntry,
    UseItem,
    UsePath,
    ConstItem,
    GlobalItem,
    FunctionItem,
    ParamList,
    Param,
    StructItem,
    StructFieldList,
    StructField,
    EnumItem,
    EnumVariantList,
    EnumVariant,
    TupleFieldList,
    RecordFieldList,
    TraitItem,
    TraitMethod,
    ImplItem,
    ImplMethod,
    TypeHint,
    TypeArgList,
    Path,
    PathSegment,
    Block,
    LetStmt,
    ReturnStmt,
    BreakStmt,
    ContinueStmt,
    ForStmt,
    IfExpr,
    MatchExpr,
    MatchArmList,
    MatchArm,
    ExprStmt,
    Literal,
    PathExpr,
    UnaryExpr,
    BinaryExpr,
    AssignExpr,
    FieldExpr,
    CallExpr,
    ArgList,
    Argument,
    IndexExpr,
    TryExpr,
    ArrayExpr,
    MapExpr,
    MapEntry,
    RecordExpr,
    RecordExprFieldList,
    RecordExprField,
    LambdaExpr,
    Pattern,
    TuplePattern,
    RecordPattern,
    RecordPatternField,
    Error,
    Whitespace,
    LineComment,
    BlockComment,
    Shebang,
    Ident,
    Int,
    Float,
    Char,
    String,
    InterpolatedString,
    Bytes,
    Unknown,
    UseKw,
    PubKw,
    ConstKw,
    GlobalKw,
    LetKw,
    FnKw,
    StructKw,
    EnumKw,
    TraitKw,
    ImplKw,
    ForKw,
    IfKw,
    ElseKw,
    MatchKw,
    ReturnKw,
    BreakKw,
    ContinueKw,
    TrueKw,
    FalseKw,
    NullKw,
    SelfKw,
    InKw,
    AsKw,
    Hash,
    LBracket,
    RBracket,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Dot,
    DotDot,
    DotDotEqual,
    Colon,
    ColonColon,
    Semicolon,
    Arrow,
    FatArrow,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BangEqual,
    BangEqualEqual,
    EqualEqual,
    EqualEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Pipe,
    Question,
    Eof,
}

const LAST_KIND: u16 = SyntaxKind::Eof as u16;

impl SyntaxKind {
    #[must_use]
    pub const fn is_node(self) -> bool {
        matches!(
            self,
            Self::SourceFile
                | Self::Attribute
                | Self::AttributeArg
                | Self::AttributeArray
                | Self::AttributeMap
                | Self::AttributeMapEntry
                | Self::UseItem
                | Self::UsePath
                | Self::ConstItem
                | Self::GlobalItem
                | Self::FunctionItem
                | Self::ParamList
                | Self::Param
                | Self::StructItem
                | Self::StructFieldList
                | Self::StructField
                | Self::EnumItem
                | Self::EnumVariantList
                | Self::EnumVariant
                | Self::TupleFieldList
                | Self::RecordFieldList
                | Self::TraitItem
                | Self::TraitMethod
                | Self::ImplItem
                | Self::ImplMethod
                | Self::TypeHint
                | Self::TypeArgList
                | Self::Path
                | Self::PathSegment
                | Self::Block
                | Self::LetStmt
                | Self::ReturnStmt
                | Self::BreakStmt
                | Self::ContinueStmt
                | Self::ForStmt
                | Self::IfExpr
                | Self::MatchExpr
                | Self::MatchArmList
                | Self::MatchArm
                | Self::ExprStmt
                | Self::Literal
                | Self::PathExpr
                | Self::UnaryExpr
                | Self::BinaryExpr
                | Self::AssignExpr
                | Self::FieldExpr
                | Self::CallExpr
                | Self::ArgList
                | Self::Argument
                | Self::IndexExpr
                | Self::TryExpr
                | Self::ArrayExpr
                | Self::MapExpr
                | Self::MapEntry
                | Self::RecordExpr
                | Self::RecordExprFieldList
                | Self::RecordExprField
                | Self::LambdaExpr
                | Self::Pattern
                | Self::TuplePattern
                | Self::RecordPattern
                | Self::RecordPatternField
                | Self::Error
        )
    }

    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment | Self::Shebang
        )
    }

    #[must_use]
    pub const fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::UseKw
                | Self::PubKw
                | Self::ConstKw
                | Self::GlobalKw
                | Self::LetKw
                | Self::FnKw
                | Self::StructKw
                | Self::EnumKw
                | Self::TraitKw
                | Self::ImplKw
                | Self::ForKw
                | Self::IfKw
                | Self::ElseKw
                | Self::MatchKw
                | Self::ReturnKw
                | Self::BreakKw
                | Self::ContinueKw
                | Self::TrueKw
                | Self::FalseKw
                | Self::NullKw
                | Self::SelfKw
                | Self::InKw
                | Self::AsKw
        )
    }

    #[must_use]
    pub const fn is_symbol(self) -> bool {
        matches!(
            self,
            Self::Hash
                | Self::LBracket
                | Self::RBracket
                | Self::LParen
                | Self::RParen
                | Self::LBrace
                | Self::RBrace
                | Self::Comma
                | Self::Dot
                | Self::DotDot
                | Self::DotDotEqual
                | Self::Colon
                | Self::ColonColon
                | Self::Semicolon
                | Self::Arrow
                | Self::FatArrow
                | Self::Equal
                | Self::PlusEqual
                | Self::MinusEqual
                | Self::StarEqual
                | Self::SlashEqual
                | Self::PercentEqual
                | Self::Plus
                | Self::Minus
                | Self::Star
                | Self::Slash
                | Self::Percent
                | Self::Bang
                | Self::BangEqual
                | Self::BangEqualEqual
                | Self::EqualEqual
                | Self::EqualEqualEqual
                | Self::Less
                | Self::LessEqual
                | Self::Greater
                | Self::GreaterEqual
                | Self::AndAnd
                | Self::OrOr
                | Self::Pipe
                | Self::Question
        )
    }

    #[must_use]
    pub const fn is_token(self) -> bool {
        !self.is_node()
    }
}

impl From<u16> for SyntaxKind {
    fn from(raw: u16) -> Self {
        assert!(raw <= LAST_KIND, "invalid Vela syntax kind raw value {raw}");
        match raw {
            0 => Self::SourceFile,
            1 => Self::Attribute,
            2 => Self::AttributeArg,
            3 => Self::AttributeArray,
            4 => Self::AttributeMap,
            5 => Self::AttributeMapEntry,
            6 => Self::UseItem,
            7 => Self::UsePath,
            8 => Self::ConstItem,
            9 => Self::GlobalItem,
            10 => Self::FunctionItem,
            11 => Self::ParamList,
            12 => Self::Param,
            13 => Self::StructItem,
            14 => Self::StructFieldList,
            15 => Self::StructField,
            16 => Self::EnumItem,
            17 => Self::EnumVariantList,
            18 => Self::EnumVariant,
            19 => Self::TupleFieldList,
            20 => Self::RecordFieldList,
            21 => Self::TraitItem,
            22 => Self::TraitMethod,
            23 => Self::ImplItem,
            24 => Self::ImplMethod,
            25 => Self::TypeHint,
            26 => Self::TypeArgList,
            27 => Self::Path,
            28 => Self::PathSegment,
            29 => Self::Block,
            30 => Self::LetStmt,
            31 => Self::ReturnStmt,
            32 => Self::BreakStmt,
            33 => Self::ContinueStmt,
            34 => Self::ForStmt,
            35 => Self::IfExpr,
            36 => Self::MatchExpr,
            37 => Self::MatchArmList,
            38 => Self::MatchArm,
            39 => Self::ExprStmt,
            40 => Self::Literal,
            41 => Self::PathExpr,
            42 => Self::UnaryExpr,
            43 => Self::BinaryExpr,
            44 => Self::AssignExpr,
            45 => Self::FieldExpr,
            46 => Self::CallExpr,
            47 => Self::ArgList,
            48 => Self::Argument,
            49 => Self::IndexExpr,
            50 => Self::TryExpr,
            51 => Self::ArrayExpr,
            52 => Self::MapExpr,
            53 => Self::MapEntry,
            54 => Self::RecordExpr,
            55 => Self::RecordExprFieldList,
            56 => Self::RecordExprField,
            57 => Self::LambdaExpr,
            58 => Self::Pattern,
            59 => Self::TuplePattern,
            60 => Self::RecordPattern,
            61 => Self::RecordPatternField,
            62 => Self::Error,
            63 => Self::Whitespace,
            64 => Self::LineComment,
            65 => Self::BlockComment,
            66 => Self::Shebang,
            67 => Self::Ident,
            68 => Self::Int,
            69 => Self::Float,
            70 => Self::Char,
            71 => Self::String,
            72 => Self::InterpolatedString,
            73 => Self::Bytes,
            74 => Self::Unknown,
            75 => Self::UseKw,
            76 => Self::PubKw,
            77 => Self::ConstKw,
            78 => Self::GlobalKw,
            79 => Self::LetKw,
            80 => Self::FnKw,
            81 => Self::StructKw,
            82 => Self::EnumKw,
            83 => Self::TraitKw,
            84 => Self::ImplKw,
            85 => Self::ForKw,
            86 => Self::IfKw,
            87 => Self::ElseKw,
            88 => Self::MatchKw,
            89 => Self::ReturnKw,
            90 => Self::BreakKw,
            91 => Self::ContinueKw,
            92 => Self::TrueKw,
            93 => Self::FalseKw,
            94 => Self::NullKw,
            95 => Self::SelfKw,
            96 => Self::InKw,
            97 => Self::AsKw,
            98 => Self::Hash,
            99 => Self::LBracket,
            100 => Self::RBracket,
            101 => Self::LParen,
            102 => Self::RParen,
            103 => Self::LBrace,
            104 => Self::RBrace,
            105 => Self::Comma,
            106 => Self::Dot,
            107 => Self::DotDot,
            108 => Self::DotDotEqual,
            109 => Self::Colon,
            110 => Self::ColonColon,
            111 => Self::Semicolon,
            112 => Self::Arrow,
            113 => Self::FatArrow,
            114 => Self::Equal,
            115 => Self::PlusEqual,
            116 => Self::MinusEqual,
            117 => Self::StarEqual,
            118 => Self::SlashEqual,
            119 => Self::PercentEqual,
            120 => Self::Plus,
            121 => Self::Minus,
            122 => Self::Star,
            123 => Self::Slash,
            124 => Self::Percent,
            125 => Self::Bang,
            126 => Self::BangEqual,
            127 => Self::BangEqualEqual,
            128 => Self::EqualEqual,
            129 => Self::EqualEqualEqual,
            130 => Self::Less,
            131 => Self::LessEqual,
            132 => Self::Greater,
            133 => Self::GreaterEqual,
            134 => Self::AndAnd,
            135 => Self::OrOr,
            136 => Self::Pipe,
            137 => Self::Question,
            138 => Self::Eof,
            _ => unreachable!("raw syntax kind already checked against LAST_KIND"),
        }
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> Self {
        kind as u16
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VelaLanguage {}

impl Language for VelaLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from(raw.0)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.into())
    }
}

#[cfg(test)]
mod tests {
    use super::{SyntaxKind, VelaLanguage};
    use rowan::Language;

    const ALL_KINDS: &[SyntaxKind] = &[
        SyntaxKind::SourceFile,
        SyntaxKind::Attribute,
        SyntaxKind::AttributeArg,
        SyntaxKind::AttributeArray,
        SyntaxKind::AttributeMap,
        SyntaxKind::AttributeMapEntry,
        SyntaxKind::UseItem,
        SyntaxKind::UsePath,
        SyntaxKind::ConstItem,
        SyntaxKind::GlobalItem,
        SyntaxKind::FunctionItem,
        SyntaxKind::ParamList,
        SyntaxKind::Param,
        SyntaxKind::StructItem,
        SyntaxKind::StructFieldList,
        SyntaxKind::StructField,
        SyntaxKind::EnumItem,
        SyntaxKind::EnumVariantList,
        SyntaxKind::EnumVariant,
        SyntaxKind::TupleFieldList,
        SyntaxKind::RecordFieldList,
        SyntaxKind::TraitItem,
        SyntaxKind::TraitMethod,
        SyntaxKind::ImplItem,
        SyntaxKind::ImplMethod,
        SyntaxKind::TypeHint,
        SyntaxKind::TypeArgList,
        SyntaxKind::Path,
        SyntaxKind::PathSegment,
        SyntaxKind::Block,
        SyntaxKind::LetStmt,
        SyntaxKind::ReturnStmt,
        SyntaxKind::BreakStmt,
        SyntaxKind::ContinueStmt,
        SyntaxKind::ForStmt,
        SyntaxKind::IfExpr,
        SyntaxKind::MatchExpr,
        SyntaxKind::MatchArmList,
        SyntaxKind::MatchArm,
        SyntaxKind::ExprStmt,
        SyntaxKind::Literal,
        SyntaxKind::PathExpr,
        SyntaxKind::UnaryExpr,
        SyntaxKind::BinaryExpr,
        SyntaxKind::AssignExpr,
        SyntaxKind::FieldExpr,
        SyntaxKind::CallExpr,
        SyntaxKind::ArgList,
        SyntaxKind::Argument,
        SyntaxKind::IndexExpr,
        SyntaxKind::TryExpr,
        SyntaxKind::ArrayExpr,
        SyntaxKind::MapExpr,
        SyntaxKind::MapEntry,
        SyntaxKind::RecordExpr,
        SyntaxKind::RecordExprFieldList,
        SyntaxKind::RecordExprField,
        SyntaxKind::LambdaExpr,
        SyntaxKind::Pattern,
        SyntaxKind::TuplePattern,
        SyntaxKind::RecordPattern,
        SyntaxKind::RecordPatternField,
        SyntaxKind::Error,
        SyntaxKind::Whitespace,
        SyntaxKind::LineComment,
        SyntaxKind::BlockComment,
        SyntaxKind::Shebang,
        SyntaxKind::Ident,
        SyntaxKind::Int,
        SyntaxKind::Float,
        SyntaxKind::Char,
        SyntaxKind::String,
        SyntaxKind::InterpolatedString,
        SyntaxKind::Bytes,
        SyntaxKind::Unknown,
        SyntaxKind::UseKw,
        SyntaxKind::PubKw,
        SyntaxKind::ConstKw,
        SyntaxKind::GlobalKw,
        SyntaxKind::LetKw,
        SyntaxKind::FnKw,
        SyntaxKind::StructKw,
        SyntaxKind::EnumKw,
        SyntaxKind::TraitKw,
        SyntaxKind::ImplKw,
        SyntaxKind::ForKw,
        SyntaxKind::IfKw,
        SyntaxKind::ElseKw,
        SyntaxKind::MatchKw,
        SyntaxKind::ReturnKw,
        SyntaxKind::BreakKw,
        SyntaxKind::ContinueKw,
        SyntaxKind::TrueKw,
        SyntaxKind::FalseKw,
        SyntaxKind::NullKw,
        SyntaxKind::SelfKw,
        SyntaxKind::InKw,
        SyntaxKind::AsKw,
        SyntaxKind::Hash,
        SyntaxKind::LBracket,
        SyntaxKind::RBracket,
        SyntaxKind::LParen,
        SyntaxKind::RParen,
        SyntaxKind::LBrace,
        SyntaxKind::RBrace,
        SyntaxKind::Comma,
        SyntaxKind::Dot,
        SyntaxKind::DotDot,
        SyntaxKind::DotDotEqual,
        SyntaxKind::Colon,
        SyntaxKind::ColonColon,
        SyntaxKind::Semicolon,
        SyntaxKind::Arrow,
        SyntaxKind::FatArrow,
        SyntaxKind::Equal,
        SyntaxKind::PlusEqual,
        SyntaxKind::MinusEqual,
        SyntaxKind::StarEqual,
        SyntaxKind::SlashEqual,
        SyntaxKind::PercentEqual,
        SyntaxKind::Plus,
        SyntaxKind::Minus,
        SyntaxKind::Star,
        SyntaxKind::Slash,
        SyntaxKind::Percent,
        SyntaxKind::Bang,
        SyntaxKind::BangEqual,
        SyntaxKind::BangEqualEqual,
        SyntaxKind::EqualEqual,
        SyntaxKind::EqualEqualEqual,
        SyntaxKind::Less,
        SyntaxKind::LessEqual,
        SyntaxKind::Greater,
        SyntaxKind::GreaterEqual,
        SyntaxKind::AndAnd,
        SyntaxKind::OrOr,
        SyntaxKind::Pipe,
        SyntaxKind::Question,
        SyntaxKind::Eof,
    ];

    #[test]
    fn syntax_kind_round_trips_rowan_raw() {
        for &kind in ALL_KINDS {
            let raw = VelaLanguage::kind_to_raw(kind);
            assert_eq!(VelaLanguage::kind_from_raw(raw), kind);
            assert_eq!(SyntaxKind::from(u16::from(kind)), kind);
        }
    }

    #[test]
    fn syntax_kind_classifies_nodes_tokens_and_trivia() {
        assert!(SyntaxKind::SourceFile.is_node());
        assert!(!SyntaxKind::SourceFile.is_token());
        assert!(SyntaxKind::Whitespace.is_trivia());
        assert!(SyntaxKind::Whitespace.is_token());
        assert!(SyntaxKind::FnKw.is_keyword());
        assert!(SyntaxKind::ColonColon.is_symbol());
        assert!(SyntaxKind::Unknown.is_token());
        assert!(SyntaxKind::Eof.is_token());
    }
}
