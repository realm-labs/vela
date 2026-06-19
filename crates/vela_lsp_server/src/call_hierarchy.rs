use vela_language_service::CallHierarchyItem;

pub(crate) fn service_call_hierarchy_item(
    item: &crate::protocol::CallHierarchyItem,
    document_text: &str,
) -> Result<CallHierarchyItem, String> {
    let line_index = crate::line_index::LineIndex::new(document_text);
    Ok(CallHierarchyItem::new(
        item.name.clone(),
        vela_language_service::DocumentId::from(item.uri.clone()),
        line_index.service_range(item.range)?,
        line_index.service_range(item.selection_range)?,
    ))
}
