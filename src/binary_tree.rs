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
                        let new_ptr = new.as_mut();
                        new_ptr.parent = Some(ptr);
                    }
                }
            }
        } else {
            self.root = Some(self.node_for_value(value));
        }
        self.len += 1;
        true
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
        let box_node = Box::new(Node {
            value: value,
            left: None,
            right: None,
            parent: None,
        });
        let raw_ptr = Box::into_raw(box_node);
        // SAFETY: we just created raw pointer to non null box
        unsafe { NonNull::new_unchecked(raw_ptr) }
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
}
