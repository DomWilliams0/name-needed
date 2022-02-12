pub struct SizedIterator<T, I: Iterator<Item = T>>(I, usize);

impl<T, I: Iterator<Item = T>> SizedIterator<T, I> {
    pub fn new(iterator: I, upper: usize) -> Self {
        Self(iterator, upper)
    }
}

impl<T, I: Iterator<Item = T>> Iterator for SizedIterator<T, I> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, _) = self.0.size_hint();
        (lower, Some(self.1))
    }
}
