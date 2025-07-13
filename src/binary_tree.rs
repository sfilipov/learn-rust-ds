use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;
use std::ptr::NonNull;

pub struct Tree<T: Ord> {
    root: Link<T>,
    len: usize,
    _phantom: PhantomData<T>,
}

struct Node<T> {
    value: T,
    parent: Link<T>,
    left: Link<T>,
    right: Link<T>,
}

type Link<T> = Option<NonNull<Node<T>>>;

impl<T: Ord> Tree<T> {
    pub fn new() -> Self {
        Tree {
            root: None,
            len: 0,
            _phantom: PhantomData,
        }
    }

    pub fn insert(&mut self, value: T) -> bool {
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
                    }
                }
            }
        } else {
            self.root = Some(self.node_for_value(value));
        }
        self.len += 1;
        true
    }

    pub fn remove(&mut self, value: &T) -> bool {
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

    pub fn contains(&self, value: &T) -> bool {
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

    pub fn len(&self) -> usize {
        self.len
    }

    fn node_for_value(&self, value: T) -> NonNull<Node<T>> {
        // SAFETY: we just created raw pointer to non null box
        unsafe {
            NonNull::new_unchecked(Box::into_raw(Box::new(Node {
                value: value,
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

    fn remove_node(&mut self, node_ptr: NonNull<Node<T>>) {
        unsafe {
            let node = node_ptr.as_ref();
            if let (Some(_), Some(_)) = (node.left, node.right) {
                let before = node.before();
                let mut before_ptr = before.expect("Node with left child should have before node");
                let before_node = before_ptr.as_mut();

                self.replace_node(before_ptr, before_node.left);
                self.replace_node(node_ptr, before);
            } else {
                let child = node.left.or(node.right);
                self.replace_node(node_ptr, child);
            }

            // recreate Box and let it be dropped automatically
            let _box_to_drop = Box::from_raw(node_ptr.as_ptr());
        }

        self.len -= 1;
    }

    fn replace_node(&mut self, node_ptr: NonNull<Node<T>>, new_link: Link<T>) {
        unsafe {
            let node = node_ptr.as_ref();
            if let Some(mut parent_ptr) = node.parent {
                let parent_node = parent_ptr.as_mut();
                if eq_link_and_ptr(parent_node.left, node_ptr) {
                    parent_node.left = new_link;
                } else {
                    parent_node.right = new_link;
                }
            } else {
                self.root = new_link;
            }

            if let Some(mut new_ptr) = new_link {
                let new_node = new_ptr.as_mut();
                new_node.parent = node.parent;

                if !eq_link_and_ptr(node.left, new_ptr) {
                    new_node.left = node.left;
                    if let Some(mut new_left_ptr) = new_node.left {
                        let new_left_node = new_left_ptr.as_mut();
                        new_left_node.parent = new_link;
                    }
                }

                if !eq_link_and_ptr(node.right, new_ptr) {
                    new_node.right = node.right;
                    if let Some(mut new_right_ptr) = new_node.right {
                        let new_right_node = new_right_ptr.as_mut();
                        new_right_node.parent = new_link;
                    }
                }
            }
        }
    }
}

fn eq_link_and_ptr<T>(a_link: Link<T>, b_ptr: NonNull<Node<T>>) -> bool {
    a_link.map_or(false, |a_ptr| unsafe {
        std::ptr::eq(a_ptr.as_ref(), b_ptr.as_ref())
    })
}

impl<T: Ord> Drop for Tree<T> {
    fn drop(&mut self) {
        while let Some(ptr) = self.root {
            self.remove_node(ptr);
        }
    }
}

impl<T> Node<T> {
    fn before(&self) -> Link<T> {
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
}

impl<T: fmt::Debug + Ord> fmt::Debug for Tree<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Tree")
            .field("len", &self.len)
            .field("root", &self.root.map(|ptr| unsafe { ptr.as_ref() }))
            .finish()
    }
}

impl<T: fmt::Debug> fmt::Debug for Node<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Node")
            .field("value", &self.value)
            .field(
                "parent",
                &self.parent.map(|ptr| unsafe { &ptr.as_ref().value }),
            )
            .field("left", &self.left.map(|ptr| unsafe { ptr.as_ref() }))
            .field("right", &self.right.map(|ptr| unsafe { ptr.as_ref() }))
            .finish()
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
                assert_eq!(tree.contains(&j), true, "{tree:#?}");
            }
        }
    }
}
