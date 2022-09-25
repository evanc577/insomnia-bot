use std::ops::{Index, IndexMut};

pub struct Array2d<T>
where
    T: Default,
{
    data: Vec<T>,
    rows: usize,
    cols: usize,
}

impl<T> Array2d<T>
where
    T: Default,
{
    pub fn new(rows: usize, cols: usize) -> Self {
        let mut data = Vec::new();
        data.resize_with(rows * cols, Default::default);
        Self { data, rows, cols }
    }
}

impl<T> Index<[usize; 2]> for Array2d<T>
where
    T: Default,
{
    type Output = T;

    fn index(&self, index: [usize; 2]) -> &Self::Output {
        assert!(index[0] < self.rows);
        assert!(index[1] < self.cols);
        &self.data[self.cols * index[0] + index[1]]
    }
}

impl<T> IndexMut<[usize; 2]> for Array2d<T>
where
    T: Default,
{
    fn index_mut(&mut self, index: [usize; 2]) -> &mut Self::Output {
        assert!(index[0] < self.rows);
        assert!(index[1] < self.cols);
        &mut self.data[self.cols * index[0] + index[1]]
    }
}
