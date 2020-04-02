#[cfg(feature = "serde")]
pub mod serde;

use std::{cmp::Ordering, iter::FromIterator};
#[derive(Debug, Clone, PartialEq)]
enum Operation {
    Delete(u32),
    Retain(u32),
    Insert(String),
}

#[derive(Debug, PartialEq)]
pub struct TextOperation {
    // The consecutive operations to be applied to the target.
    ops: Vec<Operation>,
    // The required length of a string these operations can be applied to.
    base_len: usize,
    // The length of the resulting string after the operations have been
    // applied.
    target_len: usize,
}

impl Default for TextOperation {
    fn default() -> Self {
        Self {
            ops: Vec::new(),
            base_len: 0,
            target_len: 0,
        }
    }
}

impl FromIterator<Operation> for TextOperation {
    fn from_iter<T: IntoIterator<Item = Operation>>(ops: T) -> Self {
        let mut operations = TextOperation::default();
        for op in ops {
            operations.add(op);
        }
        operations
    }
}

impl TextOperation {
    pub fn compose(&self, other: &Self) -> Self {
        assert_eq!(self.target_len, other.base_len, "The base length of the second operation has to be the target length of the first operation");

        let mut new_operations = TextOperation::default();
        let mut ops1 = self.ops.iter().cloned();
        let mut ops2 = other.ops.iter().cloned();

        let mut maybe_op1 = ops1.next();
        let mut maybe_op2 = ops2.next();
        loop {
            match (&maybe_op1, &maybe_op2) {
                (None, None) => break,
                (Some(Operation::Delete(i)), _) => {
                    new_operations.delete(*i);
                    maybe_op1 = ops1.next();
                }
                (_, Some(Operation::Insert(s))) => {
                    new_operations.insert(s.clone());
                    maybe_op2 = ops2.next();
                }
                (None, _) => {
                    panic!("Cannot compose operations: first operation is too short.");
                }
                (_, None) => {
                    panic!("Cannot compose operations: second operation is too short.");
                }
                (Some(Operation::Retain(i)), Some(Operation::Retain(j))) => match i.cmp(&j) {
                    Ordering::Less => {
                        new_operations.retain(*i);
                        maybe_op2 = Some(Operation::Retain(*j - *i));
                        maybe_op1 = ops1.next();
                    }
                    std::cmp::Ordering::Equal => {
                        new_operations.retain(*i);
                        maybe_op1 = ops1.next();
                        maybe_op2 = ops2.next();
                    }
                    std::cmp::Ordering::Greater => {
                        new_operations.retain(*j);
                        maybe_op1 = Some(Operation::Retain(*i - *j));
                        maybe_op2 = ops2.next();
                    }
                },
                (Some(Operation::Insert(s)), Some(Operation::Delete(j))) => {
                    match (s.chars().count() as u32).cmp(j) {
                        Ordering::Less => {
                            maybe_op2 = Some(Operation::Delete(*j - s.chars().count() as u32));
                            maybe_op1 = ops1.next();
                        }
                        Ordering::Equal => {
                            maybe_op1 = ops1.next();
                            maybe_op2 = ops2.next();
                        }
                        Ordering::Greater => {
                            maybe_op1 =
                                Some(Operation::Insert(s.chars().skip(*j as usize).collect()));
                            maybe_op2 = ops2.next();
                        }
                    }
                }
                (Some(Operation::Insert(s)), Some(Operation::Retain(j))) => {
                    match (s.chars().count() as u32).cmp(j) {
                        Ordering::Less => {
                            new_operations.insert(s.to_owned());
                            maybe_op2 = Some(Operation::Retain(*j - s.chars().count() as u32));
                            maybe_op1 = ops1.next();
                        }
                        Ordering::Equal => {
                            new_operations.insert(s.to_owned());
                            maybe_op1 = ops1.next();
                            maybe_op2 = ops2.next();
                        }
                        Ordering::Greater => {
                            let chars = &mut s.chars();
                            new_operations.insert(chars.take(*j as usize).collect());
                            maybe_op1 = Some(Operation::Insert(chars.collect()));
                            maybe_op2 = ops2.next();
                        }
                    }
                }
                (Some(Operation::Retain(i)), Some(Operation::Delete(j))) => match i.cmp(&j) {
                    Ordering::Less => {
                        new_operations.delete(*i);
                        maybe_op2 = Some(Operation::Delete(*j - *i));
                        maybe_op1 = ops1.next();
                    }
                    Ordering::Equal => {
                        new_operations.delete(*j);
                        maybe_op2 = ops2.next();
                        maybe_op1 = ops1.next();
                    }
                    Ordering::Greater => {
                        new_operations.delete(*j);
                        maybe_op1 = Some(Operation::Retain(*i - *j));
                        maybe_op2 = ops2.next();
                    }
                },
            };
        }
        new_operations
    }

    fn add(&mut self, op: Operation) {
        match op {
            Operation::Delete(i) => self.delete(i),
            Operation::Insert(s) => self.insert(s),
            Operation::Retain(i) => self.retain(i),
        }
    }

    pub fn delete(&mut self, i: u32) {
        if i == 0 {
            return;
        }
        self.base_len += i as usize;
        if let Some(Operation::Delete(i_last)) = self.ops.last_mut() {
            *i_last += i;
        } else {
            self.ops.push(Operation::Delete(i));
        }
    }

    pub fn insert(&mut self, s: String) {
        if s.is_empty() {
            return;
        }
        self.target_len += s.chars().count();
        let new_last = match self.ops.as_mut_slice() {
            [.., Operation::Insert(s_last)] => {
                *s_last += &s;
                return;
            }
            [.., Operation::Insert(s_pre_last), Operation::Delete(_)] => {
                *s_pre_last += &s;
                return;
            }
            [.., op_last @ Operation::Delete(_)] => {
                let new_last = op_last.clone();
                *op_last = Operation::Insert(s);
                new_last
            }
            _ => Operation::Insert(s),
        };
        self.ops.push(new_last);
    }

    pub fn retain(&mut self, i: u32) {
        if i == 0 {
            return;
        }
        self.base_len += i as usize;
        self.target_len += i as usize;
        if let Some(Operation::Retain(i_last)) = self.ops.last_mut() {
            *i_last += i;
        } else {
            self.ops.push(Operation::Retain(i));
        }
    }

    pub fn transform(&self, other: &Self) -> (Self, Self) {
        assert_eq!(
            self.base_len, other.base_len,
            "Both operations have to have the same base length"
        );
        let mut a_prime = TextOperation::default();
        let mut b_prime = TextOperation::default();

        let mut ops1 = self.ops.iter().cloned();
        let mut ops2 = other.ops.iter().cloned();

        let mut maybe_op1 = ops1.next();
        let mut maybe_op2 = ops2.next();
        loop {
            match (&maybe_op1, &maybe_op2) {
                (None, None) => break,
                (Some(Operation::Insert(s)), _) => {
                    a_prime.insert(s.to_owned());
                    b_prime.retain(s.chars().count() as _);
                    maybe_op1 = ops1.next();
                }
                (_, Some(Operation::Insert(s))) => {
                    a_prime.retain(s.chars().count() as _);
                    b_prime.insert(s.to_owned());
                    maybe_op2 = ops2.next();
                }
                (None, _) => {
                    panic!("Cannot compose operations: first operation is too short.");
                }
                (_, None) => {
                    panic!("Cannot compose operations: second operation is too short.");
                }
                (Some(Operation::Retain(i)), Some(Operation::Retain(j))) => {
                    let mut min = 0;
                    match i.cmp(&j) {
                        Ordering::Less => {
                            min = *i;
                            maybe_op2 = Some(Operation::Retain(*j - *i));
                            maybe_op1 = ops1.next();
                        }
                        Ordering::Equal => {
                            min = *i;
                            maybe_op1 = ops1.next();
                            maybe_op2 = ops2.next();
                        }
                        Ordering::Greater => {
                            min = *j;
                            maybe_op1 = Some(Operation::Retain(*i - *j));
                            maybe_op2 = ops2.next();
                        }
                    };
                    a_prime.retain(min);
                    b_prime.retain(min);
                }
                (Some(Operation::Delete(i)), Some(Operation::Delete(j))) => match i.cmp(&j) {
                    Ordering::Less => {
                        maybe_op2 = Some(Operation::Delete(*j - *i));
                        maybe_op1 = ops1.next();
                    }
                    Ordering::Equal => {
                        maybe_op1 = ops1.next();
                        maybe_op2 = ops2.next();
                    }
                    Ordering::Greater => {
                        maybe_op1 = Some(Operation::Delete(*i - *j));
                        maybe_op2 = ops2.next();
                    }
                },
                (Some(Operation::Delete(i)), Some(Operation::Retain(j))) => {
                    let mut min = 0;
                    match i.cmp(&j) {
                        Ordering::Less => {
                            min = *i;
                            maybe_op2 = Some(Operation::Retain(*j - *i));
                            maybe_op1 = ops1.next();
                        }
                        Ordering::Equal => {
                            min = *i;
                            maybe_op1 = ops1.next();
                            maybe_op2 = ops2.next();
                        }
                        Ordering::Greater => {
                            min = *j;
                            maybe_op1 = Some(Operation::Delete(*i - *j));
                            maybe_op2 = ops2.next();
                        }
                    };
                    a_prime.delete(min);
                }
                (Some(Operation::Retain(i)), Some(Operation::Delete(j))) => {
                    let mut min = 0;
                    match i.cmp(&j) {
                        Ordering::Less => {
                            min = *i;
                            maybe_op2 = Some(Operation::Delete(*j - *i));
                            maybe_op1 = ops1.next();
                        }
                        Ordering::Equal => {
                            min = *i;
                            maybe_op1 = ops1.next();
                            maybe_op2 = ops2.next();
                        }
                        Ordering::Greater => {
                            min = *j;
                            maybe_op1 = Some(Operation::Retain(*i - *j));
                            maybe_op2 = ops2.next();
                        }
                    };
                    b_prime.delete(min);
                }
            }
        }

        (a_prime, b_prime)
    }

    pub fn apply(&self, s: &str) -> String {
        assert_eq!(
            s.chars().count(),
            self.base_len,
            "The operation's base length must be equal to the string's length."
        );
        let mut new_s = String::new();
        let chars = &mut s.chars();
        for op in self.ops.iter() {
            match op {
                Operation::Retain(retain) => {
                    for c in chars.take(*retain as usize) {
                        new_s.push(c);
                    }
                }
                Operation::Delete(delete) => {
                    for _ in 0..*delete {
                        chars.next();
                    }
                }
                Operation::Insert(insert) => {
                    new_s += insert;
                }
            }
        }
        new_s
    }

    pub fn invert(&self, s: &str) -> Self {
        let mut inverse = TextOperation::default();
        let chars = &mut s.chars();
        for op in self.ops.iter() {
            match op {
                Operation::Retain(retain) => {
                    inverse.retain(*retain);
                    for _ in 0..*retain {
                        chars.next();
                    }
                }
                Operation::Insert(insert) => {
                    inverse.delete(insert.chars().count() as u32);
                }
                Operation::Delete(delete) => {
                    inverse.insert(chars.take(*delete as usize).collect::<String>());
                }
            }
        }
        inverse
    }

    pub fn is_noop(&self) -> bool {
        match self.ops.as_slice() {
            [] => true,
            [Operation::Retain(_)] => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;

    fn random_string(len: usize) -> String {
        (0..len).map(|_| rand::random::<char>()).collect()
    }

    fn random_text_operation(s: &str) -> TextOperation {
        let mut op = TextOperation::default();
        let mut rng = rand::thread_rng();
        loop {
            let left = s.chars().count() - op.base_len;
            if left == 0 {
                break;
            }
            let i = if left == 1 {
                1
            } else {
                1 + rng.gen_range(0, std::cmp::min(left - 1, 20))
            };
            match rng.gen_range(0.0, 1.0) {
                f if f < 0.2 => {
                    op.insert(random_string(i));
                }
                f if f < 0.4 => {
                    op.delete(i as u32);
                }
                _ => {
                    op.retain(i as u32);
                }
            }
        }
        if rng.gen_range(0.0, 1.0) < 0.3 {
            op.insert("1".to_owned() + &random_string(10));
        }
        op
    }

    #[test]
    fn lengths() {
        let mut o = TextOperation::default();
        assert_eq!(o.base_len, 0);
        assert_eq!(o.target_len, 0);
        o.retain(5);
        assert_eq!(o.base_len, 5);
        assert_eq!(o.target_len, 5);
        o.insert("abc".to_owned());
        assert_eq!(o.base_len, 5);
        assert_eq!(o.target_len, 8);
        o.retain(2);
        assert_eq!(o.base_len, 7);
        assert_eq!(o.target_len, 10);
        o.delete(2);
        assert_eq!(o.base_len, 9);
        assert_eq!(o.target_len, 10);
    }

    #[test]
    fn sequence() {
        let mut o = TextOperation::default();
        o.retain(5);
        o.retain(0);
        o.insert("lorem".to_owned());
        o.insert("".to_owned());
        o.delete(3);
        o.delete(0);
        assert_eq!(o.ops.len(), 3);
    }

    #[test]
    fn apply() {
        for _ in 0..1000 {
            let s = random_string(50);
            let o = random_text_operation(&s);
            assert_eq!(s.chars().count(), o.base_len);
            assert_eq!(o.apply(&s).chars().count(), o.target_len);
        }
    }

    #[test]
    fn invert() {
        for _ in 0..1000 {
            let s = random_string(50);
            let o = random_text_operation(&s);
            let p = o.invert(&s);
            assert_eq!(o.base_len, p.target_len);
            assert_eq!(o.target_len, p.base_len);
            assert_eq!(p.apply(&o.apply(&s)), s);
        }
    }

    #[test]
    fn empty_ops() {
        let mut o = TextOperation::default();
        o.retain(0);
        o.insert("".to_owned());
        o.delete(0);
        assert_eq!(o.ops.len(), 0);
    }

    #[test]
    fn eq() {
        let mut o1 = TextOperation::default();
        o1.delete(1);
        o1.insert("lo".to_owned());
        o1.retain(2);
        o1.retain(3);
        let mut o2 = TextOperation::default();
        o2.delete(1);
        o2.insert("l".to_owned());
        o2.insert("o".to_owned());
        o2.retain(5);
        assert_eq!(o1, o2);
        o1.delete(1);
        o2.retain(1);
        assert_ne!(o1, o2);
    }

    #[test]
    fn ops_merging() {
        let mut o = TextOperation::default();
        assert_eq!(o.ops.len(), 0);
        o.retain(2);
        assert_eq!(o.ops.len(), 1);
        assert_eq!(o.ops.last(), Some(&Operation::Retain(2)));
        o.retain(3);
        assert_eq!(o.ops.len(), 1);
        assert_eq!(o.ops.last(), Some(&Operation::Retain(5)));
        o.insert("abc".to_owned());
        assert_eq!(o.ops.len(), 2);
        assert_eq!(o.ops.last(), Some(&Operation::Insert("abc".to_owned())));
        o.insert("xyz".to_owned());
        assert_eq!(o.ops.len(), 2);
        assert_eq!(o.ops.last(), Some(&Operation::Insert("abcxyz".to_owned())));
        o.delete(1);
        assert_eq!(o.ops.len(), 3);
        assert_eq!(o.ops.last(), Some(&Operation::Delete(1)));
        o.delete(1);
        assert_eq!(o.ops.len(), 3);
        assert_eq!(o.ops.last(), Some(&Operation::Delete(2)));
    }

    #[test]
    fn is_noop() {
        let mut o = TextOperation::default();
        assert!(o.is_noop());
        o.retain(5);
        assert!(o.is_noop());
        o.retain(3);
        assert!(o.is_noop());
        o.insert("lorem".to_owned());
        assert!(!o.is_noop());
    }

    #[test]
    fn compose() {
        for _ in 0..1000 {
            let s = random_string(20);
            let a = random_text_operation(&s);
            let after_a = a.apply(&s);
            assert_eq!(a.target_len, after_a.chars().count());
            let b = random_text_operation(&after_a);
            let after_b = b.apply(&after_a);
            assert_eq!(b.target_len, after_b.chars().count());
            let ab = a.compose(&b);
            assert_eq!(ab.target_len, b.target_len);
            let after_ab = ab.apply(&s);
            assert_eq!(after_b, after_ab);
        }
    }

    #[test]
    fn transform() {
        for _ in 0..1000 {
            let s = random_string(20);
            let a = random_text_operation(&s);
            let b = random_text_operation(&s);
            let (a_prime, b_prime) = a.transform(&b);
            let ab_prime = a.compose(&b_prime);
            let ba_prime = b.compose(&a_prime);
            let after_ab_prime = ab_prime.apply(&s);
            let after_ba_prime = ba_prime.apply(&s);
            assert_eq!(ab_prime, ba_prime);
            assert_eq!(after_ab_prime, after_ba_prime);
        }
    }

    #[test]
    #[cfg(feature = "serde")]
    fn serde() {
        use serde_json;

        let o: TextOperation = serde_json::from_str("[1,-1,\"abc\"]").unwrap();
        let mut o_exp = TextOperation::default();
        o_exp.retain(1);
        o_exp.delete(1);
        o_exp.insert("abc".to_owned());
        assert_eq!(o, o_exp);
        for _ in 0..1000 {
            let s = random_string(20);
            let o = random_text_operation(&s);
            assert_eq!(
                o,
                serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap()
            );
        }
    }
}