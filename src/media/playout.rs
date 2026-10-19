use std::collections::VecDeque;

#[derive(Default)]
pub struct FrameQueue {
    // shortcut: frames follow arrival order; use RTP sequence numbers if reordering is observed.
    frames: VecDeque<Vec<f32>>,
    ready: bool,
}

impl FrameQueue {
    pub fn push(&mut self, frame: Vec<f32>) {
        self.frames.push_back(frame);
        if self.frames.len() > 5 {
            self.frames.pop_front();
        }
        if self.frames.len() >= 2 {
            self.ready = true;
        }
    }

    pub fn pop(&mut self) -> Option<Vec<f32>> {
        if !self.ready {
            return None;
        }
        let frame = self.frames.pop_front();
        if self.frames.is_empty() {
            self.ready = false;
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::FrameQueue;

    #[test]
    fn preserves_bursts_and_discards_old_audio_when_delayed() {
        let mut queue = FrameQueue::default();
        queue.push(vec![1.0]);
        assert_eq!(queue.pop(), None);
        queue.push(vec![2.0]);
        queue.push(vec![3.0]);
        assert_eq!(queue.pop(), Some(vec![1.0]));
        assert_eq!(queue.pop(), Some(vec![2.0]));
        assert_eq!(queue.pop(), Some(vec![3.0]));
        assert_eq!(queue.pop(), None);

        for sample in 0..8 {
            queue.push(vec![sample as f32]);
        }
        for expected in 3..8 {
            assert_eq!(queue.pop(), Some(vec![expected as f32]));
        }
        assert_eq!(queue.pop(), None);
    }
}
