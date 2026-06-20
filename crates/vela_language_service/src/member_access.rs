use std::collections::BTreeMap;

use vela_syntax::ast::{AstNode, SyntaxCallExpr, SyntaxFieldExpr, SyntaxSourceFile};
use vela_syntax::{Parse as SyntaxParse, TextRange as SyntaxTextRange, TextSize};

use crate::TextRange;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MemberAccessSite {
    pub(crate) member: String,
    pub(crate) member_range: TextRange,
    pub(crate) receiver_range: TextRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct MemberCallSite {
    pub(crate) member: String,
    pub(crate) member_range: TextRange,
    pub(crate) receiver_range: TextRange,
}

pub(crate) fn member_receiver_ranges(
    parsed: &SyntaxParse<SyntaxSourceFile>,
) -> BTreeMap<(usize, usize), TextRange> {
    member_access_sites(parsed)
        .into_iter()
        .map(|site| {
            (
                (site.member_range.start, site.member_range.end),
                site.receiver_range,
            )
        })
        .collect()
}

pub(crate) fn member_call_sites(parsed: &SyntaxParse<SyntaxSourceFile>) -> Vec<MemberCallSite> {
    let source = parsed.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxCallExpr::cast)
        .filter_map(member_call_site_for_call)
        .collect()
}

pub(crate) fn member_access_sites(parsed: &SyntaxParse<SyntaxSourceFile>) -> Vec<MemberAccessSite> {
    let source = parsed.tree();
    source
        .syntax()
        .descendants()
        .filter_map(SyntaxFieldExpr::cast)
        .filter_map(member_access_site_for_field)
        .collect()
}

fn member_call_site_for_call(call: SyntaxCallExpr) -> Option<MemberCallSite> {
    let field = call.callee()?.as_field()?;
    let access = member_access_site_for_field(field)?;
    Some(MemberCallSite {
        member: access.member,
        member_range: access.member_range,
        receiver_range: access.receiver_range,
    })
}

fn member_access_site_for_field(field: SyntaxFieldExpr) -> Option<MemberAccessSite> {
    let receiver = field.receiver()?;
    let name = field.name_token()?;
    Some(MemberAccessSite {
        member: name.text().to_owned(),
        member_range: text_range(name.text_range()),
        receiver_range: text_range(receiver.syntax().text_range()),
    })
}

fn text_range(range: SyntaxTextRange) -> TextRange {
    TextRange::new(
        text_size_to_usize(range.start()),
        text_size_to_usize(range.end()),
    )
}

fn text_size_to_usize(size: TextSize) -> usize {
    u32::from(size) as usize
}

#[cfg(test)]
mod tests {
    use vela_syntax::parse::parse_source;

    use super::*;

    #[test]
    fn member_receiver_ranges_come_from_field_expression_spans() {
        let source = "\
fn main(player: Player) {
    let level = player.level
    player.grant(level)
}";
        let parsed = parse_source(source);

        let ranges = member_receiver_ranges(&parsed);

        let player_start = source.find("player.level").expect("field receiver");
        let call_receiver_start = source.find("player.grant").expect("method receiver");
        let level_start = player_start + "player.".len();
        let grant_start = call_receiver_start + "player.".len();
        assert_eq!(
            ranges.get(&(level_start, level_start + "level".len())),
            Some(&TextRange::new(player_start, player_start + "player".len()))
        );
        assert_eq!(
            ranges.get(&(grant_start, grant_start + "grant".len())),
            Some(&TextRange::new(
                call_receiver_start,
                call_receiver_start + "player".len()
            ))
        );
    }

    #[test]
    fn member_call_sites_come_from_call_callee_spans() {
        let source = "\
fn main(player: Player) {
    player.grant(player.level)
}";
        let parsed = parse_source(source);

        let calls = member_call_sites(&parsed);

        let call_receiver_start = source.find("player.grant").expect("method receiver");
        let grant_start = call_receiver_start + "player.".len();
        assert_eq!(
            calls,
            vec![MemberCallSite {
                member: "grant".to_owned(),
                member_range: TextRange::new(grant_start, grant_start + "grant".len()),
                receiver_range: TextRange::new(
                    call_receiver_start,
                    call_receiver_start + "player".len()
                ),
            }]
        );
    }

    #[test]
    fn member_access_sites_include_field_and_method_members() {
        let source = "\
fn main(player: Player) {
    player.grant(player.level)
}";
        let parsed = parse_source(source);

        let sites = member_access_sites(&parsed);

        let grant_receiver_start = source.find("player.grant").expect("method receiver");
        let grant_start = grant_receiver_start + "player.".len();
        let level_receiver_start = source.find("player.level").expect("field receiver");
        let level_start = level_receiver_start + "player.".len();
        assert_eq!(
            sites,
            vec![
                MemberAccessSite {
                    member: "grant".to_owned(),
                    member_range: TextRange::new(grant_start, grant_start + "grant".len()),
                    receiver_range: TextRange::new(
                        grant_receiver_start,
                        grant_receiver_start + "player".len()
                    ),
                },
                MemberAccessSite {
                    member: "level".to_owned(),
                    member_range: TextRange::new(level_start, level_start + "level".len()),
                    receiver_range: TextRange::new(
                        level_receiver_start,
                        level_receiver_start + "player".len()
                    ),
                },
            ]
        );
    }
}
