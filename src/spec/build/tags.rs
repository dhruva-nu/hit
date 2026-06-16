//! Tag grouping for the built spec.

use crate::model::{Endpoint, TagGroup};
use crate::spec::raw::RawSpec;

/// Tag groups: declared spec order first, then first-seen order from
/// endpoints, with an "untagged" bucket at the end when needed.
pub(super) fn group_tags(raw: &RawSpec, endpoints: &[Endpoint]) -> Vec<TagGroup> {
    let mut groups: Vec<TagGroup> = raw
        .tags
        .iter()
        .map(|t| TagGroup {
            name: t.name.clone(),
            description: t.description.clone(),
            endpoint_ids: Vec::new(),
        })
        .collect();

    let mut untagged = TagGroup {
        name: "untagged".to_string(),
        description: None,
        endpoint_ids: Vec::new(),
    };

    for endpoint in endpoints {
        if endpoint.tags.is_empty() {
            untagged.endpoint_ids.push(endpoint.id.clone());
            continue;
        }
        for tag in &endpoint.tags {
            match groups.iter_mut().find(|g| &g.name == tag) {
                Some(group) => group.endpoint_ids.push(endpoint.id.clone()),
                None => groups.push(TagGroup {
                    name: tag.clone(),
                    description: None,
                    endpoint_ids: vec![endpoint.id.clone()],
                }),
            }
        }
    }

    groups.retain(|g| !g.endpoint_ids.is_empty());
    if !untagged.endpoint_ids.is_empty() {
        groups.push(untagged);
    }
    groups
}
