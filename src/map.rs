use std::collections::{hash_map, HashMap};
use std::fmt::Display;
use std::hash::Hash;

#[derive(Debug)]
pub struct BidirectionalMap<K1, K2> {
    map1: HashMap<K1, K2>,
    map2: HashMap<K2, K1>,
}

impl<K1, K2> Display for BidirectionalMap<K1, K2>
where
    K1: Display,
    K2: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut iterations = 0;
        for (apples, oranges) in self.map1.iter() {
            iterations += 1;
            write!(f, "{apples} <=> {oranges}")?;
            if iterations != self.map1.len() {
                writeln!(f)?;
            }
        }
        Ok(())
    }
}

impl<K1, K2> FromIterator<(K1, K2)> for BidirectionalMap<K1, K2>
where
    K1: Eq + Hash + Clone,
    K2: Eq + Hash + Clone,
{
    fn from_iter<T: IntoIterator<Item = (K1, K2)>>(iter: T) -> Self {
        let (map1, map2) = iter.into_iter().fold(
            (HashMap::new(), HashMap::new()),
            |(mut a_to_o, mut o_to_a), (apples, oranges)| {
                a_to_o.insert(apples.clone(), oranges.clone());
                o_to_a.insert(oranges, apples);
                (a_to_o, o_to_a)
            },
        );
        BidirectionalMap { map1, map2 }
    }
}

impl<K1, K2> BidirectionalMap<K1, K2>
where
    K1: Eq + Hash,
    K2: Eq + Hash,
{
    pub fn get_first<Q>(&self, k: &Q) -> Option<&K1>
    where
        K2: std::borrow::Borrow<Q>,
        Q: Hash + Eq,
        Q: ?Sized,
    {
        self.map2.get(k)
    }

    pub fn get_second<Q>(&self, k: &Q) -> Option<&K2>
    where
        K1: std::borrow::Borrow<Q>,
        Q: Hash + Eq,
        Q: ?Sized,
    {
        self.map1.get(k)
    }

    pub fn insert(&mut self, k1: K1, k2: K2) -> Option<(K1, K2)>
    where
        K1: Clone,
        K2: Clone,
    {
        let old_k2 = self.map1.insert(k1.clone(), k2.clone());
        let old_k1 = self.map2.insert(k2, k1);
        old_k1.zip(old_k2)
    }
}

impl<K1, K2> BidirectionalMap<K1, K2> {
    pub fn len(&self) -> usize {
        self.map1.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map1.is_empty()
    }
    pub fn iter(&self) -> hash_map::Iter<K1, K2> {
        self.map1.iter()
    }
}

impl<K1, K2> IntoIterator for BidirectionalMap<K1, K2> {
    type Item = (K1, K2);

    type IntoIter = hash_map::IntoIter<K1, K2>;

    fn into_iter(self) -> Self::IntoIter {
        self.map1.into_iter()
    }
}

impl<'a, K1, K2> IntoIterator for &'a BidirectionalMap<K1, K2> {
    type Item = (&'a K1, &'a K2);

    type IntoIter = hash_map::Iter<'a, K1, K2>;

    fn into_iter(self) -> Self::IntoIter {
        self.map1.iter()
    }
}
