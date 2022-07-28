use std::collections::BTreeMap;

#[derive(PartialOrd, PartialEq, Clone, Copy)]
pub enum Source {
    OKX,
}

#[derive(Clone)]
pub struct Level {
    pub price: f64,
    pub size: f64,
}

impl Level {
    fn is_zero(&self) -> bool {
        self.size.abs() < 1e-5
    }
}

#[derive(Clone)]
struct LevelInfo {
    level: Level,
    source: Source,
}

pub struct Orderbook {
    asks: Vec<u64, LevelInfo>,
    bids: BTreeMap<u64, LevelInfo>,
    precision: u64,
    depth: usize,
}

pub enum Operation {
    Snapshot {
        asks: Vec<Level>,
        bids: Vec<Level>,
        source: Source,
    },
    Update {
        asks: Vec<Level>,
        bids: Vec<Level>,
        source: Source,
    },
}

impl Orderbook {
    fn process_asks(&mut self, source: Source, asks: &Vec<Level>) {
        let mut count_insert = 0;
        for ask in asks {
            if !ask.is_zero() {
                count_insert += 1;
            }
            self.asks.insert((ask.price * self.precision as f64) as u64, LevelInfo {
                level: ask.clone(),
                source: source.clone(),
            });
            if self.depth > 0 && count_insert > self.depth {
                break;
            }
        }
        if self.depth > 0 && self.asks.len() > self.depth {
            self.asks = (&self.asks).into_iter().filter(|(_, level)| !level.level.is_zero()).
                take(self.depth).map(|(k, v)| (*k, v.clone())).collect();
        }
    }

    fn process_bids(&mut self, source: Source, bids: &Vec<Level>) {
        let mut count_insert = 0;
        for bid in bids {
            if !bid.is_zero() {
                count_insert += 1;
            }
            self.bids.insert(-(bid.price * self.precision as f64) as u64, LevelInfo {
                level: bid.clone(),
                source: source.clone(),
            });
            if self.depth > 0 && count_insert > self.depth {
                break;
            }
        }
        if self.depth > 0 && self.bids.len() > self.depth {
            self.bids = (&self.bids).into_iter().filter(|(_, level)| !level.level.is_zero()).
                take(self.depth).map(|(k, v)| (*k, v.clone())).collect();
        }
    }

    pub fn process(&mut self, operation: Operation) {
        match operation {
            Operation::Snapshot { asks, bids, source } => {
                self.asks.retain(|x, y| y.source != source);
                self.bids.retain(|x, y| y.source != source);

                self.process_asks(source, &asks);
                self.process_bids(source, &bids);
            }
            Operation::Update { asks, bids, source } => {
                self.process_asks(source, &asks);
                self.process_bids(source, &bids);
            }
        }
    }
}
