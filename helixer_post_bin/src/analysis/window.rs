use crate::analysis::BasePredictionIterator;
use crate::results::conv::{Bases, Prediction};
use std::collections::VecDeque;
use std::collections::vec_deque::Iter;

pub struct BasePredictionWindow<'a>
{
    bp_iter: BasePredictionIterator<'a>,

    window_size: usize,
    scale: f32,
    window_total: u64,

    window: VecDeque<(Bases, Prediction)>,
    position: usize
}

impl<'a> BasePredictionWindow<'a>
{
    pub fn new(bp_iter: BasePredictionIterator<'a>, window_size: usize, scale: f32) -> Option<BasePredictionWindow<'a>>
    {
        let window = VecDeque::with_capacity(window_size);

        let mut bp_window = BasePredictionWindow { bp_iter, window_size, scale, window_total: 0, window, position: 0 };
        bp_window.fill_window();

        Some(bp_window)
    }

    fn fill_window(&mut self) -> bool
    {
        while self.window.len() < self.window_size
        { if !self.push() { return false; } }
        true
    }

    fn push(&mut self) -> bool
    {
        let maybe_next = self.bp_iter.next();

        if let Some((bases, pred)) = maybe_next
        {
            self.window_total+= (pred.get_genic() * self.scale) as u64;
            self.window.push_back((bases, pred));
            true
        }
        else
        { false }

    }

    fn pop(&mut self) -> Option<(Bases, Prediction)>
    {
        let maybe_next = self.window.pop_front();

        if let Some((_bases, pred)) = &maybe_next
        {
            self.window_total -= (pred.get_genic() * self.scale) as u64;
            self.position+=1;
        }

        maybe_next
    }

    pub fn get_window_total(&self) -> u64 { self.window_total }

    pub fn is_window_full(&self) -> bool { self.window.len()==self.window_size }

    pub fn get_window_iter(&self) -> Iter<(Bases, Prediction)> { self.window.iter() }
}



pub struct BasePredictionWindowThresholdScanner<'a>
{
    bp_window: BasePredictionWindow<'a>,
    edge_threshold: u64
}


impl<'a> BasePredictionWindowThresholdScanner<'a>
{
    pub fn new(bp_window: BasePredictionWindow<'a>, edge_threshold: f32) -> BasePredictionWindowThresholdScanner<'a>
    {
        let threshold = (edge_threshold * bp_window.scale * (bp_window.window_size as f32)) as u64;

        BasePredictionWindowThresholdScanner { bp_window, edge_threshold: threshold }
    }

    fn scan_for_start(&mut self) -> bool
    {
        while self.bp_window.is_window_full() && self.bp_window.get_window_total() < self.edge_threshold
        {
            self.bp_window.pop(); // Pop and discard
            self.bp_window.push();
        }

        self.bp_window.is_window_full()
    }

    fn accumulate_above_threshold(&mut self) -> (Vec<(Bases, Prediction)>, Vec<u64>, usize, u64)
    {
        let mut accum=Vec::new();
        let mut total_accum=Vec::new();

        if !self.bp_window.is_window_full() || self.bp_window.get_window_total() < self.edge_threshold
        { panic!("Accumulate called with window not past threshold"); }

        let mut peak = 0;
        let position = self.bp_window.position;

        while self.bp_window.is_window_full() && self.bp_window.get_window_total() >= self.edge_threshold
        {
            let total = self.bp_window.get_window_total();
            total_accum.push(total);
            peak = std::cmp::max(peak, total);

            let (bases, pred) = self.bp_window.pop().unwrap();
            accum.push((bases, pred));

            self.bp_window.push();
        }

        for (b,p) in self.bp_window.get_window_iter()
        { accum.push((*b, *p)) }

        if self.bp_window.is_window_full()  // If window is still full, the last element crossed below the threshold, remove it
        { accum.pop(); }

        (accum, total_accum, position, peak)
    }

}


const THRESHOLD_SCALE: f32 = 1_000_000.0;

pub struct BasePredictionWindowThresholdIterator<'a>
{
    bp_scanner: BasePredictionWindowThresholdScanner<'a>,
    peak_threshold: u64,
    peak_scale: f32
}

impl<'a> BasePredictionWindowThresholdIterator<'a>
{
    pub fn new(bp_iter: BasePredictionIterator<'a>, window_size: usize, edge_threshold: f32, peak_threshold: f32) -> Option<BasePredictionWindowThresholdIterator<'a>>
    {
        let bp_window = BasePredictionWindow::new(bp_iter, window_size, THRESHOLD_SCALE);
        if bp_window.is_none()
        { return None; }

        let bp_window=bp_window.unwrap();
        let bp_scanner = BasePredictionWindowThresholdScanner::new(bp_window, edge_threshold);

        let window_size = window_size as f32;

        let peak_threshold = (peak_threshold * THRESHOLD_SCALE * window_size) as u64;
        let peak_scale = 1.0 / (THRESHOLD_SCALE*window_size);

        Some (BasePredictionWindowThresholdIterator { bp_scanner, peak_threshold, peak_scale })
    }
}

impl<'a> Iterator for BasePredictionWindowThresholdIterator<'a>
{
    type Item = (Vec<(Bases, Prediction)>, Vec<u64>, usize, f32);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if !self.bp_scanner.scan_for_start()
            { return None; }

            let (bp_accum, total_accum, position, peak) = self.bp_scanner.accumulate_above_threshold();

            if peak > self.peak_threshold
            {
                let peak = (peak as f32)*self.peak_scale;
                return Some((bp_accum, total_accum, position, peak)); }
        }

    }
}
