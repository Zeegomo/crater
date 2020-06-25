use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use std::collections::HashMap;

pub(crate) fn hashmap_deterministic_serialize<S: Serializer, K: Ord + Serialize, V: Serialize>(
    data: &HashMap<K, V>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(data.len()))?;
    let mut data = data.iter().collect::<Vec<_>>();
    // coming from a HashMap there won't be equal keys
    data.sort_unstable_by_key(|v| v.0);
    for (k, v) in data {
        map.serialize_entry(k, v)?;
    }
    map.end()
}

pub(crate) fn sort_vec<S: Serializer, V: Ord + Serialize>(
    data: &Vec<V>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(data.len()))?;
    let mut data = data.iter().collect::<Vec<_>>();
    data.sort();
    for element in data {
        seq.serialize_element(element)?;
    }
    seq.end()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;
    use rand::{self, distributions::Alphanumeric, Rng};
    use std::collections::HashMap;

    #[derive(Serialize)]
    struct TestHashMapSerialize {
        #[serde(flatten, serialize_with = "hashmap_deterministic_serialize")]
        test_field: HashMap<String, bool>,
    }

    #[derive(Serialize)]
    struct TestVecSerialize {
        #[serde(serialize_with = "sort_vec")]
        test_field: Vec<u32>,
    }

    #[test]
    fn test_serialize_hashmap() {
        let random = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(10000)
            .collect::<Vec<_>>();
        let test_field = random
            .chunks(10)
            .map(|v| (v.iter().collect::<String>(), true))
            .collect::<HashMap<_, _>>();
        let test = TestHashMapSerialize { test_field };

        let json = serde_json::to_string(&test)
            .unwrap()
            .replace(|s| "{}:".contains(s), "");
        let elems = json.split(',').collect::<Vec<_>>();
        let mut sorted = elems.clone();
        sorted.sort();

        assert_eq!(sorted, elems);
    }

    #[test]
    fn test_serialize_vec() {
        if cfg!(test) {
            println!("cacca");
        } else {
            println!("c3a");
        }
        assert!(false);
        let unsorted = TestVecSerialize {
            test_field: (0..10).rev().collect(),
        };

        let expected = TestVecSerialize {
            test_field: (0..10).collect(),
        };

        assert_eq!(
            serde_json::to_string(&unsorted).unwrap(),
            serde_json::to_string(&expected).unwrap()
        );
    }
}
