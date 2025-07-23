use crate::tree::TreeOps;
use std::cmp::Ordering;
use std::mem::replace;

pub struct Tree<T: Ord> {
    items: Vec<Slot<T>>,
    head_free: Option<usize>,
    root: Option<usize>,
    len: usize,
}

struct Node<T> {
    value: T,
    height: i32,
    parent: Option<usize>,
    left: Option<usize>,
    right: Option<usize>,
}

enum Slot<T> {
    Occupied { node: Node<T> },
    Free { next_free: Option<usize> },
}

pub struct IntoIter<T: Ord> {
    tree: Tree<T>,
}

pub struct Iter<'a, T: Ord> {
    tree: &'a Tree<T>,
    next: Option<usize>,
}

impl<T: Ord> TreeOps<T> for Tree<T> {
    fn insert(&mut self, value: T) -> bool {
        let closest = self.find_closest(&value);
        if let Some(index) = closest {
            let node = self.unwrap_occupied(index);
            match value.cmp(&node.value) {
                Ordering::Equal => return false,
                ord => {
                    let new = self.insert_node(value, Some(index));
                    let node = self.unwrap_occupied_mut(index);
                    if ord == Ordering::Less {
                        node.left = Some(new);
                    } else {
                        node.right = Some(new);
                    }
                    self.update_ancestor_heights(closest);
                    self.rebalance_ancestors(closest);
                }
            }
        } else {
            self.root = Some(self.insert_node(value, None));
        }
        self.len += 1;
        true
    }

    fn remove(&mut self, value: &T) -> bool {
        let closest = self.find_closest(&value);
        let Some(index) = closest else {
            return false;
        };

        let node = self.unwrap_occupied(index);
        if value.cmp(&node.value) != Ordering::Equal {
            return false;
        }

        self.remove_node(index);
        true
    }

    fn contains(&self, value: &T) -> bool {
        let closest = self.find_closest(value);
        if let Some(index) = closest {
            let node = self.unwrap_occupied(index);
            value.cmp(&node.value) == Ordering::Equal
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
            items: Vec::new(),
            head_free: None,
            root: None,
            len: 0,
        }
    }

    #[cfg(test)]
    fn height(&self) -> i32 {
        match self.root {
            None => -1,
            Some(index) => self.unwrap_occupied(index).height,
        }
    }

    pub fn into_iter(self) -> IntoIter<T> {
        IntoIter { tree: self }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            tree: self,
            next: self.first(),
        }
    }

    fn find_closest(&self, value: &T) -> Option<usize> {
        let mut prev = None;
        let mut cur = self.root;
        while let Some(index) = cur {
            prev = cur;
            let node = self.unwrap_occupied(index);
            match value.cmp(&node.value) {
                Ordering::Less => cur = node.left,
                Ordering::Greater => cur = node.right,
                Ordering::Equal => return cur,
            }
        }
        return prev;
    }

    fn insert_node(&mut self, value: T, parent: Option<usize>) -> usize {
        let mut node = Node::new(value);
        node.parent = parent;

        if let Some(free_index) = self.head_free {
            if let Slot::Free { next_free } = self.items[free_index] {
                self.head_free = next_free;
                self.items[free_index] = Slot::Occupied { node };
                free_index
            } else {
                unreachable!("Corrupted arena");
            }
        } else {
            self.items.push(Slot::Occupied { node });
            self.items.len() - 1
        }
    }

    fn remove_node(&mut self, index: usize) -> T {
        let (node_left, node_right, node_parent) = {
            let node = self.unwrap_occupied(index);
            (node.left, node.right, node.parent)
        };

        if let (Some(_), Some(_)) = (node_left, node_right) {
            let before = self.before_sub(index);
            let before_index = before.expect("Node with left child should have before node");
            let before_node = self.unwrap_occupied(before_index);

            let ancestor_start = if before == node_left {
                before
            } else {
                before_node.parent
            };

            self.replace_node(before_index, before_node.left);
            self.replace_node(index, before);

            self.update_ancestor_heights(ancestor_start);
            self.rebalance_ancestors(ancestor_start);
        } else {
            let child = node_left.or(node_right);
            self.replace_node(index, child);

            self.update_ancestor_heights(node_parent);
            self.rebalance_ancestors(node_parent);
        }

        let value = self.remove_node_from_arena(index);
        self.len -= 1;
        value
    }

    fn replace_node(&mut self, node_index: usize, new_link: Option<usize>) {
        let (node_parent, node_left, node_right) = {
            let node = self.unwrap_occupied(node_index);
            (node.parent, node.left, node.right)
        };
        if let Some(parent_index) = node_parent {
            let parent_node_left = self.unwrap_occupied(parent_index).left;
            if parent_node_left == Some(node_index) {
                self.with_occupied_mut(parent_index, |parent| parent.left = new_link);
            } else {
                self.with_occupied_mut(parent_index, |parent| parent.right = new_link);
            }
        } else {
            self.root = new_link;
        }

        if let Some(new_index) = new_link {
            self.with_occupied_mut(new_index, |new_node| new_node.parent = node_parent);

            if node_left != new_link {
                self.with_occupied_mut(new_index, |new_node| new_node.left = node_left);
                if let Some(child_index) = self.unwrap_occupied(new_index).left {
                    self.with_occupied_mut(child_index, |child| child.parent = new_link);
                }
            }

            if node_right != new_link {
                self.with_occupied_mut(new_index, |new_node| new_node.right = node_right);
                if let Some(child_index) = self.unwrap_occupied(new_index).right {
                    self.with_occupied_mut(child_index, |child| child.parent = new_link);
                }
            }
        }
    }

    fn remove_node_from_arena(&mut self, index: usize) -> T {
        let slot = replace(
            &mut self.items[index],
            Slot::Free {
                next_free: self.head_free,
            },
        );
        self.head_free = Some(index);
        let Slot::Occupied { node } = slot else {
            unreachable!("Corrupted arena");
        };
        node.value
    }

    fn first(&self) -> Option<usize> {
        let mut cur = self.root;
        while let Some(cur_index) = cur {
            match self.before(cur_index) {
                None => return cur,
                before => cur = before,
            }
        }
        None
    }

    fn rebalance_ancestors(&mut self, link: Option<usize>) {
        let mut cur = link;
        while let Some(index) = cur {
            self.rebalance(cur);
            cur = self.unwrap_occupied(index).parent;
        }
    }

    fn rebalance(&mut self, link: Option<usize>) {
        let Some(index) = link else {
            return;
        };
        let node = self.unwrap_occupied(index);
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

    fn rotate_right(&mut self, x_link: Option<usize>) -> Option<usize> {
        let Some(x_index) = x_link else {
            return None;
        };
        let (x_left, x_parent) = {
            let x = self.unwrap_occupied(x_index);
            (x.left, x.parent)
        };
        let y_link = x_left;
        let Some(y_index) = y_link else {
            return None;
        };
        let t2_link = self.unwrap_occupied(y_index).right;

        // fix parent -> y
        if let Some(parent_index) = x_parent {
            let parent_node_right = self.unwrap_occupied(parent_index).right;
            if parent_node_right == x_link {
                self.with_occupied_mut(parent_index, |parent| parent.right = y_link);
            } else {
                self.with_occupied_mut(parent_index, |parent| parent.left = y_link);
            }
        } else {
            self.root = y_link;
        }

        self.with_occupied_mut(y_index, |y| y.parent = x_parent);

        // fix y -> x
        self.with_occupied_mut(x_index, |x| x.parent = y_link);
        self.with_occupied_mut(y_index, |y| y.right = x_link);

        // fix x -> t2
        self.with_occupied_mut(x_index, |x| x.left = t2_link);
        if let Some(t2_index) = t2_link {
            self.with_occupied_mut(t2_index, |t2| t2.parent = x_link);
        }
        y_link
    }

    fn rotate_left(&mut self, x_link: Option<usize>) -> Option<usize> {
        let Some(x_index) = x_link else {
            return None;
        };
        let (x_right, x_parent) = {
            let x = self.unwrap_occupied(x_index);
            (x.right, x.parent)
        };
        let y_link = x_right;
        let Some(y_index) = y_link else {
            return None;
        };
        let t2_link = self.unwrap_occupied(y_index).left;

        // fix parent -> y
        if let Some(parent_index) = x_parent {
            let parent_node_left = self.unwrap_occupied(parent_index).left;
            if parent_node_left == x_link {
                self.with_occupied_mut(parent_index, |parent| parent.left = y_link);
            } else {
                self.with_occupied_mut(parent_index, |parent| parent.right = y_link);
            }
        } else {
            self.root = y_link;
        }

        self.with_occupied_mut(y_index, |y| y.parent = x_parent);

        // fix y -> x
        self.with_occupied_mut(x_index, |x| x.parent = y_link);
        self.with_occupied_mut(y_index, |y| y.left = x_link);

        // fix x -> t2
        self.with_occupied_mut(x_index, |x| x.right = t2_link);
        if let Some(t2_index) = t2_link {
            self.with_occupied_mut(t2_index, |t2| t2.parent = x_link);
        }
        y_link
    }

    fn update_ancestor_heights(&mut self, link: Option<usize>) {
        let mut cur = link;
        while let Some(index) = cur {
            self.update_height(index);
            cur = self.unwrap_occupied(index).parent;
        }
    }

    fn update_height(&mut self, index: usize) {
        let node = self.unwrap_occupied(index);
        let left_height = self.link_height(node.left);
        let right_height = self.link_height(node.right);

        let node = self.unwrap_occupied_mut(index);
        node.height = 1 + left_height.max(right_height);
    }

    fn balance_factor(&self, link: Option<usize>) -> i32 {
        if let Some(index) = link {
            let node = self.unwrap_occupied(index);
            let left_height = self.link_height(node.left);
            let right_height = self.link_height(node.right);
            return left_height - right_height;
        } else {
            0
        }
    }

    fn link_height(&self, link: Option<usize>) -> i32 {
        match link {
            Some(index) => self.unwrap_occupied(index).height,
            None => -1,
        }
    }

    fn before(&self, index: usize) -> Option<usize> {
        self.before_sub(index).or(self.before_above(index))
    }

    fn before_sub(&self, index: usize) -> Option<usize> {
        let node = self.unwrap_occupied(index);
        let Some(mut cur) = node.left else {
            return None;
        };

        while let Some(right) = self.unwrap_occupied(cur).right {
            cur = right;
        }
        Some(cur)
    }

    fn before_above(&self, index: usize) -> Option<usize> {
        let node = self.unwrap_occupied(index);
        let mut cur_index = index;
        let mut cur = node;
        while let Some(parent_index) = cur.parent {
            let parent = self.unwrap_occupied(parent_index);
            if parent.left == Some(cur_index) {
                cur_index = parent_index;
                cur = parent;
            } else {
                return cur.parent;
            }
        }
        None
    }

    fn after(&self, index: usize) -> Option<usize> {
        self.after_sub(index).or(self.after_above(index))
    }

    fn after_sub(&self, index: usize) -> Option<usize> {
        let node = self.unwrap_occupied(index);
        let Some(mut cur) = node.right else {
            return None;
        };

        while let Some(left) = self.unwrap_occupied(cur).left {
            cur = left;
        }
        Some(cur)
    }

    fn after_above(&self, index: usize) -> Option<usize> {
        let node = self.unwrap_occupied(index);
        let mut cur_index = index;
        let mut cur = node;
        while let Some(parent_index) = cur.parent {
            let parent = self.unwrap_occupied(parent_index);
            if parent.right == Some(cur_index) {
                cur_index = parent_index;
                cur = parent;
            } else {
                return cur.parent;
            }
        }
        None
    }

    fn unwrap_occupied(&self, index: usize) -> &Node<T> {
        match &self.items[index] {
            Slot::Occupied { node } => node,
            Slot::Free { .. } => panic!("Called unwrap_occupied on free slot"),
        }
    }

    fn unwrap_occupied_mut(&mut self, index: usize) -> &mut Node<T> {
        match &mut self.items[index] {
            Slot::Occupied { node } => node,
            Slot::Free { .. } => panic!("Called unwrap_occupied on free slot"),
        }
    }

    fn with_occupied_mut<F>(&mut self, index: usize, f: F)
    where
        F: FnOnce(&mut Node<T>),
    {
        match &mut self.items[index] {
            Slot::Occupied { node } => f(node),
            Slot::Free { .. } => panic!("Called unwrap_occupied on free slot"),
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
        self.next.map(|index| {
            let node = self.tree.unwrap_occupied(index);
            self.next = self.tree.after(index);
            &node.value
        })
    }
}

impl<T: Ord> Drop for Tree<T> {
    fn drop(&mut self) {
        while let Some(ptr) = self.root {
            self.remove_node(ptr);
        }
    }
}

impl<T: Ord> Node<T> {
    pub fn new(value: T) -> Self {
        Node {
            value,
            height: 0,
            parent: None,
            left: None,
            right: None,
        }
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
            tree.first().map(|index| tree.unwrap_occupied(index).value),
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
            tree.first().map(|index| tree.unwrap_occupied(index).value),
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
}
