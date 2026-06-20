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
    ParenExpr,
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
                | Self::ParenExpr
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
            42 => Self::ParenExpr,
            43 => Self::UnaryExpr,
            44 => Self::BinaryExpr,
            45 => Self::AssignExpr,
            46 => Self::FieldExpr,
            47 => Self::CallExpr,
            48 => Self::ArgList,
            49 => Self::Argument,
            50 => Self::IndexExpr,
            51 => Self::TryExpr,
            52 => Self::ArrayExpr,
            53 => Self::MapExpr,
            54 => Self::MapEntry,
            55 => Self::RecordExpr,
            56 => Self::RecordExprFieldList,
            57 => Self::RecordExprField,
            58 => Self::LambdaExpr,
            59 => Self::Pattern,
            60 => Self::TuplePattern,
            61 => Self::RecordPattern,
            62 => Self::RecordPatternField,
            63 => Self::Error,
            64 => Self::Whitespace,
            65 => Self::LineComment,
            66 => Self::BlockComment,
            67 => Self::Shebang,
            68 => Self::Ident,
            69 => Self::Int,
            70 => Self::Float,
            71 => Self::Char,
            72 => Self::String,
            73 => Self::InterpolatedString,
            74 => Self::Bytes,
            75 => Self::Unknown,
            76 => Self::UseKw,
            77 => Self::PubKw,
            78 => Self::ConstKw,
            79 => Self::GlobalKw,
            80 => Self::LetKw,
            81 => Self::FnKw,
            82 => Self::StructKw,
            83 => Self::EnumKw,
            84 => Self::TraitKw,
            85 => Self::ImplKw,
            86 => Self::ForKw,
            87 => Self::IfKw,
            88 => Self::ElseKw,
            89 => Self::MatchKw,
            90 => Self::ReturnKw,
            91 => Self::BreakKw,
            92 => Self::ContinueKw,
            93 => Self::TrueKw,
            94 => Self::FalseKw,
            95 => Self::NullKw,
            96 => Self::SelfKw,
            97 => Self::InKw,
            98 => Self::AsKw,
            99 => Self::Hash,
            100 => Self::LBracket,
            101 => Self::RBracket,
            102 => Self::LParen,
            103 => Self::RParen,
            104 => Self::LBrace,
            105 => Self::RBrace,
            106 => Self::Comma,
            107 => Self::Dot,
            108 => Self::DotDot,
            109 => Self::DotDotEqual,
            110 => Self::Colon,
            111 => Self::ColonColon,
            112 => Self::Semicolon,
            113 => Self::Arrow,
            114 => Self::FatArrow,
            115 => Self::Equal,
            116 => Self::PlusEqual,
            117 => Self::MinusEqual,
            118 => Self::StarEqual,
            119 => Self::SlashEqual,
            120 => Self::PercentEqual,
            121 => Self::Plus,
            122 => Self::Minus,
            123 => Self::Star,
            124 => Self::Slash,
            125 => Self::Percent,
            126 => Self::Bang,
            127 => Self::BangEqual,
            128 => Self::BangEqualEqual,
            129 => Self::EqualEqual,
            130 => Self::EqualEqualEqual,
            131 => Self::Less,
            132 => Self::LessEqual,
            133 => Self::Greater,
            134 => Self::GreaterEqual,
            135 => Self::AndAnd,
            136 => Self::OrOr,
            137 => Self::Pipe,
            138 => Self::Question,
            139 => Self::Eof,
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
