pub struct HeapQ<T: Ord> {
    values: Vec<T>,
}

impl<T: Ord> HeapQ<T> {
    pub fn new() -> Self {
        HeapQ { values: Vec::new() }
    }

    pub fn push(&mut self, value: T) {
        self.values.push(value);
        self.siftup(self.values.len() - 1);
    }

    pub fn pop(&mut self) -> Option<T> {
        match self.values.len() {
            0 => None,
            1 => self.values.pop(),
            x => {
                self.values.swap(0, x - 1);
                let value = self.values.pop();
                self.siftdown(0);
                value
            }
        }
    }

    pub fn top(&mut self) -> Option<&T> {
        self.values.get(0)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    fn siftup(&mut self, mut index: usize) {
        while index > 0 {
            let parent = (index - 1) / 2;
            if self.values[parent] < self.values[index] {
                self.values.swap(index, parent);
                index = parent;
            } else {
                break;
            }
        }
    }

    fn siftdown(&mut self, mut index: usize) {
        while index < self.values.len() {
            let left_idx = 2 * index + 1;
            let right_idx = 2 * index + 2;
            if left_idx >= self.values.len() {
                break;
            }

            let mut greater_idx = left_idx;
            if right_idx < self.values.len() && self.values[left_idx] < self.values[right_idx] {
                greater_idx = right_idx;
            }

            if self.values[index] < self.values[greater_idx] {
                self.values.swap(index, greater_idx);
                index = greater_idx;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::seq::SliceRandom;

    use super::HeapQ;

    #[test]
    fn empty_after_creation() {
        let heap = HeapQ::<i32>::new();
        assert_eq!(heap.len(), 0);
        assert_eq!(heap.is_empty(), true);
    }

    #[test]
    fn length_after_push() {
        let mut heap = HeapQ::new();
        heap.push(100);
        assert_eq!(heap.len(), 1);
        assert_eq!(heap.is_empty(), false);
    }

    #[test]
    fn correct_top_pop_after_push() {
        let mut heap = HeapQ::new();
        assert_eq!(heap.top(), None);
        heap.push(1);
        assert_eq!(*heap.top().unwrap(), 1);
        heap.push(3);
        assert_eq!(*heap.top().unwrap(), 3);

        // check 3 (the greatest element) is still top after pushing 2
        heap.push(2);
        assert_eq!(*heap.top().unwrap(), 3);
    }

    #[test]
    fn top_after_many_shuffled_inserts() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut numbers: Vec<i32> = (0..10).collect();
        numbers.shuffle(&mut rng);

        let mut heap = HeapQ::new();
        for &number in numbers.iter() {
            heap.push(number);
        }

        for i in (0..10).rev() {
            assert_eq!(*heap.top().unwrap(), i);
            assert_eq!(heap.len(), (i + 1) as usize);
            assert_eq!(heap.pop().unwrap(), i);
        }
    }
}
