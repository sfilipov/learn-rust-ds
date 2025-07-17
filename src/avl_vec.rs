use crate::tree::TreeOps;
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub struct Tree<T: Ord> {
    root: Link<T>,
    len: usize,
    _marker: PhantomData<T>,
}

struct Node<T> {
    value: T,
    height: i32,
    parent: Link<T>,
    left: Link<T>,
    right: Link<T>,
}

type Link<T> = Option<NonNull<Node<T>>>;

pub struct IntoIter<T: Ord> {
    tree: Tree<T>,
}

pub struct Iter<'a, T: Ord> {
    next: Link<T>,
    _marker: PhantomData<&'a T>,
}

pub struct IterMut<'a, T: Ord> {
    next: Link<T>,
    _marker: PhantomData<&'a mut T>,
}

impl<T: Ord> TreeOps<T> for Tree<T> {
    fn insert(&mut self, value: T) -> bool {
        let closest = self.find_closest(&value);
        if let Some(mut ptr) = closest {
            // SAFETY: we only create valid NonNulls from node_for_value function
            unsafe {
                let node = ptr.as_mut();

                match value.cmp(&node.value) {
                    Ordering::Equal => return false,
                    ord => {
                        let mut new = self.node_for_value(value);
                        if ord == Ordering::Less {
                            node.left = Some(new);
                        } else {
                            node.right = Some(new);
                        }
                        let new_node = new.as_mut();
                        new_node.parent = Some(ptr);
                        self.update_ancestor_heights(closest);
                        self.rebalance_ancestors(closest);
                    }
                }
            }
        } else {
            self.root = Some(self.node_for_value(value));
        }
        self.len += 1;
        true
    }

    fn remove(&mut self, value: &T) -> bool {
        let closest = self.find_closest(&value);
        let Some(ptr) = closest else {
            return false;
        };

        unsafe {
            let node = ptr.as_ref();
            if value.cmp(&node.value) != Ordering::Equal {
                return false;
            }
        }

        self.remove_node(ptr);
        return true;
    }

    fn contains(&self, value: &T) -> bool {
        let closest = self.find_closest(value);
        if let Some(ptr) = closest {
            // SAFETY: we only create valid NonNulls from node_for_value function
            unsafe {
                let node = ptr.as_ref();
                value.cmp(&node.value) == Ordering::Equal
            }
        } else {
            false
        }
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl<T: Ord> Tree<T> {
    pub fn new() -> Self {
        Tree {
            root: None,
            len: 0,
            _marker: PhantomData,
        }
    }

    #[cfg(test)]
    fn height(&self) -> i32 {
        match self.root {
            None => -1,
            Some(ptr) => unsafe { ptr.as_ref().height },
        }
    }

    pub fn into_iter(self) -> IntoIter<T> {
        IntoIter { tree: self }
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        Iter {
            next: self.first(),
            _marker: PhantomData,
        }
    }

    pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, T> {
        IterMut {
            next: self.first(),
            _marker: PhantomData,
        }
    }

    fn node_for_value(&self, value: T) -> NonNull<Node<T>> {
        // SAFETY: we just created raw pointer to non null box
        unsafe {
            NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                value: value,
                height: 0,
                left: None,
                right: None,
                parent: None,
            })))
        }
    }

    fn find_closest(&self, value: &T) -> Link<T> {
        let mut prev = None;
        let mut cur = self.root;
        while let Some(ptr) = cur {
            prev = cur;
            // SAFETY: we only create valid NonNulls from node_for_value function
            unsafe {
                let node = ptr.as_ref();
                match value.cmp(&node.value) {
                    Ordering::Less => cur = node.left,
                    Ordering::Greater => cur = node.right,
                    Ordering::Equal => return cur,
                }
            }
        }
        return prev;
    }

    fn remove_node(&mut self, node_ptr: NonNull<Node<T>>) -> T {
        unsafe {
            let node = node_ptr.as_ref();
            if let (Some(_), Some(_)) = (node.left, node.right) {
                let before = node.before_sub();
                let before_ptr = before.expect("Node with left child should have before node");
                let before_node = before_ptr.as_ref();

                let ancestor_start = if before == node.left {
                    before
                } else {
                    before_node.parent
                };

                self.replace_node(before_ptr, before_node.left);
                self.replace_node(node_ptr, before);

                self.update_ancestor_heights(ancestor_start);
                self.rebalance_ancestors(ancestor_start);
            } else {
                let child = node.left.or(node.right);
                self.replace_node(node_ptr, child);

                self.update_ancestor_heights(node.parent);
                self.rebalance_ancestors(node.parent);
            }

            // recreate Box and let it be dropped automatically
            let _box_to_drop = Box::from_raw(node_ptr.as_ptr());
            let value = _box_to_drop.value;
            self.len -= 1;
            value
        }
    }

    fn replace_node(&mut self, node_ptr: NonNull<Node<T>>, new_link: Link<T>) {
        unsafe {
            let node = node_ptr.as_ref();
            if let Some(mut parent_ptr) = node.parent {
                let parent_node = parent_ptr.as_mut();
                if eq_link_and_node(parent_node.left, node) {
                    parent_node.left = new_link;
                } else {
                    parent_node.right = new_link;
                }
            } else {
                self.root = new_link;
            }

            if let Some(mut new_ptr) = new_link {
                new_ptr.as_mut().parent = node.parent;

                if !eq_link_and_node(node.left, new_ptr.as_ref()) {
                    new_ptr.as_mut().left = node.left;
                    if let Some(mut new_left_ptr) = new_ptr.as_ref().left {
                        new_left_ptr.as_mut().parent = new_link;
                    }
                }

                if !eq_link_and_node(node.right, new_ptr.as_ref()) {
                    new_ptr.as_mut().right = node.right;
                    if let Some(mut new_right_ptr) = new_ptr.as_ref().right {
                        new_right_ptr.as_mut().parent = new_link;
                    }
                }
            }
        }
    }

    fn first(&self) -> Link<T> {
        unsafe {
            let mut cur = self.root;
            while let Some(cur_ptr) = cur {
                match cur_ptr.as_ref().before() {
                    None => return cur,
                    before => cur = before,
                }
            }
            None
        }
    }

    fn rebalance_ancestors(&mut self, link: Link<T>) {
        let mut cur = link;
        while let Some(ptr) = cur {
            self.rebalance(cur);
            unsafe {
                cur = ptr.as_ref().parent;
            }
        }
    }

    fn rebalance(&mut self, link: Link<T>) {
        unsafe {
            let Some(ptr) = link else {
                return;
            };
            let node = ptr.as_ref();
            let balance_factor = self.balance_factor(link);
            if balance_factor > 1 {
                let mut height_start = link;
                if self.balance_factor(node.left) < 0 {
                    height_start = node.left;
                    self.rotate_left(node.left);
                }
                self.rotate_right(link);
                self.update_ancestor_heights(height_start);
            } else if balance_factor < -1 {
                let mut height_start = link;
                if self.balance_factor(node.right) > 0 {
                    height_start = node.right;
                    self.rotate_right(node.right);
                }
                self.rotate_left(link);
                self.update_ancestor_heights(height_start);
            }
        }
    }

    fn rotate_right(&mut self, x_link: Link<T>) -> Link<T> {
        unsafe {
            let Some(mut x_ptr) = x_link else {
                return None;
            };
            let x = x_ptr.as_mut();
            let y_link = x.left;
            let Some(mut y_ptr) = y_link else {
                return None;
            };
            let y = y_ptr.as_mut();
            let t2 = y.right;

            // fix parent -> y
            if let Some(mut parent_ptr) = x.parent {
                let parent_node = parent_ptr.as_mut();
                if eq_link_and_node(parent_node.left, x) {
                    parent_node.left = y_link;
                } else {
                    parent_node.right = y_link;
                }
            } else {
                self.root = y_link;
            }
            y.parent = x.parent;

            // fix y -> x
            x.parent = y_link;
            y.right = x_link;

            // fix x -> t2
            x.left = t2;
            if let Some(mut t2_ptr) = t2 {
                let t2_node = t2_ptr.as_mut();
                t2_node.parent = x_link;
            }
            y_link
        }
    }

    fn rotate_left(&mut self, x_link: Link<T>) -> Link<T> {
        unsafe {
            let Some(mut x_ptr) = x_link else {
                return None;
            };
            let x = x_ptr.as_mut();
            let y_link = x.right;
            let Some(mut y_ptr) = y_link else {
                return None;
            };
            let y = y_ptr.as_mut();
            let t2 = y.left;

            // fix parent -> y
            if let Some(mut parent_ptr) = x.parent {
                let parent_node = parent_ptr.as_mut();
                if eq_link_and_node(parent_node.left, x) {
                    parent_node.left = y_link;
                } else {
                    parent_node.right = y_link;
                }
            } else {
                self.root = y_link;
            }
            y.parent = x.parent;

            // fix y -> x
            x.parent = y_link;
            y.left = x_link;

            // fix x -> t2
            x.right = t2;
            if let Some(mut t2_ptr) = t2 {
                let t2_node = t2_ptr.as_mut();
                t2_node.parent = x_link;
            }
            y_link
        }
    }

    fn update_ancestor_heights(&self, link: Link<T>) {
        let mut cur = link;
        while let Some(ptr) = cur {
            self.update_height(cur);
            unsafe {
                cur = ptr.as_ref().parent;
            }
        }
    }

    fn update_height(&self, link: Link<T>) {
        if let Some(mut ptr) = link {
            unsafe {
                let node = ptr.as_mut();
                let left_height = self.link_height(node.left);
                let right_height = self.link_height(node.right);
                node.height = 1 + left_height.max(right_height);
            }
        }
    }

    fn balance_factor(&self, link: Link<T>) -> i32 {
        if let Some(ptr) = link {
            unsafe {
                let node = ptr.as_ref();
                let left_height = self.link_height(node.left);
                let right_height = self.link_height(node.right);
                return left_height - right_height;
            }
        } else {
            0
        }
    }

    fn link_height(&self, link: Link<T>) -> i32 {
        match link {
            Some(ptr) => unsafe { ptr.as_ref().height },
            None => -1,
        }
    }
}

impl<T: Ord> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.tree.first().map(|ptr| self.tree.remove_node(ptr))
    }
}

impl<'a, T: Ord> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.next.map(|ptr| {
                let node = ptr.as_ref();
                self.next = node.after();
                &node.value
            })
        }
    }
}

impl<'a, T: Ord> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            self.next.map(|mut ptr| {
                self.next = ptr.as_ref().after();
                &mut ptr.as_mut().value
            })
        }
    }
}

fn eq_link_and_node<T: Ord>(a_link: Link<T>, b_ptr: &Node<T>) -> bool {
    a_link.map_or(false, |a_ptr| std::ptr::eq(a_ptr.as_ptr(), b_ptr))
}

impl<T: Ord> Drop for Tree<T> {
    fn drop(&mut self) {
        while let Some(ptr) = self.root {
            self.remove_node(ptr);
        }
    }
}

impl<T: Ord> Node<T> {
    fn before(&self) -> Link<T> {
        self.before_sub().or(self.before_above())
    }

    fn before_sub(&self) -> Link<T> {
        let Some(mut cur) = self.left else {
            return None;
        };

        unsafe {
            while let Some(right) = cur.as_ref().right {
                cur = right;
            }
        }
        Some(cur)
    }

    fn before_above(&self) -> Link<T> {
        let mut cur = self;
        while let Some(parent_ptr) = cur.parent {
            unsafe {
                let parent = parent_ptr.as_ref();
                if eq_link_and_node(parent.left, cur) {
                    cur = parent;
                } else {
                    return cur.parent;
                }
            }
        }
        None
    }

    fn after(&self) -> Link<T> {
        self.after_sub().or(self.after_above())
    }

    fn after_sub(&self) -> Link<T> {
        let Some(mut cur) = self.right else {
            return None;
        };

        unsafe {
            while let Some(left) = cur.as_ref().left {
                cur = left;
            }
        }
        Some(cur)
    }

    fn after_above(&self) -> Link<T> {
        let mut cur = self;
        while let Some(parent_ptr) = cur.parent {
            unsafe {
                let parent = parent_ptr.as_ref();
                if eq_link_and_node(parent.right, cur) {
                    cur = parent;
                } else {
                    return cur.parent;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_after_creation() {
        let tree = Tree::<i32>::new();
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn insert_and_contains() {
        let mut tree = Tree::<i32>::new();
        for i in 0..10 {
            assert_eq!(tree.len(), i as usize);
            tree.insert(i);
            assert!(tree.contains(&i));
        }
        for i in 0..10 {
            assert!(tree.contains(&i));
        }
        assert!(!tree.contains(&100));
    }

    #[test]
    fn insert_large_logarithmic_height() {
        let mut tree = Tree::<i32>::new();
        let size = 1000;
        for i in 0..size {
            assert_eq!(tree.len(), i as usize);
            tree.insert(i);
            assert!(tree.contains(&i));
        }
        for i in 0..size {
            assert!(tree.contains(&i));
        }

        assert_eq!(tree.height(), size.ilog2() as i32);
    }

    #[test]
    fn remove_and_contains() {
        let mut tree = Tree::<i32>::new();
        for i in 0..10 {
            tree.insert(i);
        }
        for i in 0..10 {
            assert!(tree.contains(&i));
        }
        for i in 0..10 {
            for j in 0..i {
                assert_eq!(tree.contains(&j), false);
            }
            for j in i..10 {
                assert_eq!(tree.contains(&j), true);
            }

            let removed = tree.remove(&i);
            assert_eq!(removed, true);

            for j in 0..i + 1 {
                assert_eq!(tree.contains(&j), false);
            }
            for j in i + 1..10 {
                assert_eq!(tree.contains(&j), true);
            }
        }
    }

    #[test]
    fn first_after_asc_insert() {
        let mut tree = Tree::new();
        for i in 0..10 {
            tree.insert(i);
        }
        assert_eq!(
            tree.first().map(|ptr| unsafe { ptr.as_ref().value }),
            Some(0)
        );
    }

    #[test]
    fn first_after_desc_insert() {
        let mut tree = Tree::new();
        for i in (0..10).rev() {
            tree.insert(i);
        }
        assert_eq!(
            tree.first().map(|ptr| unsafe { ptr.as_ref().value }),
            Some(0)
        );
    }

    #[test]
    fn into_iter_asc() {
        let mut tree = Tree::new();
        for i in 0..10 {
            tree.insert(i);
        }

        let mut iter = tree.into_iter();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(i));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn into_iter_desc() {
        let mut tree = Tree::new();
        for i in (0..10).rev() {
            tree.insert(i);
        }

        let mut iter = tree.into_iter();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(i));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_asc() {
        let mut tree = Tree::new();
        for i in 0..10 {
            tree.insert(i);
        }

        let mut iter = tree.iter();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(&i));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_desc() {
        let mut tree = Tree::new();
        for i in (0..10).rev() {
            tree.insert(i);
        }

        let mut iter = tree.iter();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(&i));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_mut_asc() {
        let mut tree = Tree::new();
        for i in 0..10 {
            tree.insert(i);
        }

        let mut iter = tree.iter_mut();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(&mut i.clone()));
        }
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_mut_desc() {
        let mut tree = Tree::new();
        for i in (0..10).rev() {
            tree.insert(i);
        }

        let mut iter = tree.iter_mut();
        for i in 0..10 {
            assert_eq!(iter.next(), Some(&mut i.clone()));
        }
        assert_eq!(iter.next(), None);
    }
}
