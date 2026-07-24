use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use vela_host::object::{ScriptHostFieldAccess, ScriptHostObject};

use super::standard_collection_host_type_id;
use crate::standard::standard_slice_host_type_id;

#[test]
fn standard_collection_host_views_share_the_sealed_binding_identity() {
    let vec = vec![vec![1_i64]];
    let fixed = [vec![1_i64], vec![2]];
    let btree_map = BTreeMap::from([("key".to_owned(), vec![1_i64])]);
    let hash_map = HashMap::from([("key".to_owned(), vec![1_i64])]);
    let btree_set = BTreeSet::from(["key".to_owned()]);
    let hash_set = HashSet::from(["key".to_owned()]);
    let slice = vec.as_slice();

    assert_eq!(
        vec.host_type_id(),
        standard_collection_host_type_id::<Vec<Vec<i64>>>()
    );
    assert_eq!(
        fixed.host_type_id(),
        standard_collection_host_type_id::<[Vec<i64>; 2]>()
    );
    assert_eq!(
        btree_map.host_type_id(),
        standard_collection_host_type_id::<BTreeMap<String, Vec<i64>>>()
    );
    assert_eq!(
        hash_map.host_type_id(),
        standard_collection_host_type_id::<HashMap<String, Vec<i64>>>()
    );
    assert_eq!(
        btree_set.host_type_id(),
        standard_collection_host_type_id::<BTreeSet<String>>()
    );
    assert_eq!(
        hash_set.host_type_id(),
        standard_collection_host_type_id::<HashSet<String>>()
    );
    assert_eq!(
        ScriptHostFieldAccess::script_host_type_id(slice),
        standard_slice_host_type_id::<Vec<i64>>()
    );
}
