use std::collections::{
    BTreeMap,
    HashMap,
};

const EPSILON: f64 = 1e-5;

#[derive(PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
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
        self.size.abs() < EPSILON
    }
}

#[derive(Clone)]
struct LevelInfo {
    price: f64,
    source_size: HashMap<Source, f64>,
}

pub struct Orderbook {
    asks: Vec<LevelInfo>,
    bids: Vec<LevelInfo>,
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
    fn process_side(
        self_book: &mut Vec<LevelInfo>, source: Source, update_book: &Vec<Level>, depth: usize,
        side_multiplier: f64,
    ) -> Vec<LevelInfo> {
        let mut resp: Vec<LevelInfo> = Vec::with_capacity(depth);
        let mut self_it = 0;
        let mut update_it = 0;
        let self_len = self_book.len();
        let update_len = update_book.len();

        while self_it < self_len && update_it < update_len && (depth == 0 || resp.len() < depth) {
            if (self_book[self_it].price - update_book[update_it].price).abs() < EPSILON {
                self_book[self_it].source_size.insert(source, update_book[update_it].size);

                if let Some(size) = self_book[self_it].source_size.get(&source) {
                    if size.abs() < EPSILON {
                        self_book[self_it].source_size.remove(&source);
                    }
                }
                if !self_book[self_it].source_size.is_empty() {
                    resp.push(self_book[self_it].clone());
                }
                self_it += 1;
                update_it += 1;
            } else if self_book[self_it].price * side_multiplier < update_book[update_it].price * side_multiplier {
                resp.push(self_book[self_it].clone());
                self_it += 1;
            } else {
                if update_book[update_it].size > EPSILON {
                    let mut source_size = HashMap::new();
                    source_size.insert(source, update_book[update_it].size);
                    resp.push(LevelInfo { price: update_book[update_it].price, source_size });
                }
                update_it += 1;
            }
        }

        while self_it < self_len && (depth == 0 || resp.len() < depth) {
            resp.push(self_book[self_it].clone());
            self_it += 1;
        }

        while update_it < update_len && (depth == 0 || resp.len() < depth) {
            let mut source_size = HashMap::new();
            source_size.insert(source, update_book[update_it].size);
            resp.push(LevelInfo { price: update_book[update_it].price, source_size });
            update_it += 1;
        }

        resp
    }

    fn process_asks(&mut self, source: Source, asks: &Vec<Level>) {
        self.asks = Orderbook::process_side(
            &mut self.asks, source, asks, self.depth, 1.0,
        );
    }

    fn process_bids(&mut self, source: Source, bids: &Vec<Level>) {
        self.bids = Orderbook::process_side(
            &mut self.bids, source, bids, self.depth, -1.0,
        );
    }

    pub fn process(&mut self, operation: Operation) {
        match operation {
            Operation::Snapshot { asks, bids, source } => {
                self.asks = self.asks.iter_mut().map(|x| {
                    x.source_size.remove(&source);
                    x.clone()
                }).filter(|x| !x.source_size.is_empty()).collect();
                self.bids = self.bids.iter_mut().map(|mut x| {
                    x.source_size.remove(&source);
                    x.clone()
                }).filter(|x| !x.source_size.is_empty()).collect();

                self.process_asks(source, &asks);
                self.process_bids(source, &bids);
            }
            Operation::Update { asks, bids, source } => {
                self.process_asks(source, &asks);
                self.process_bids(source, &bids);
            }
        }
    }

    pub fn len(&self) -> (usize, usize) {
        (self.asks.len(), self.bids.len())
    }

    pub fn new(depth: usize) -> Self {
        Self {
            asks: vec![],
            bids: vec![],
            depth,
        }
    }
}

mod test {
    use crate::orderbook::{Level, Operation, Orderbook, Source};

    #[test]
    fn should_insert_two_on_each_side() {
        // Given
        let mut orderbook = Orderbook::new(5);
        assert_eq!((0, 0), orderbook.len());

        // When
        orderbook.process(Operation::Update {
            asks: vec![
                Level { price: 8476.98, size: 1.0 },
                Level { price: 8477.0, size: 1.0 },
            ],
            bids: vec![
                Level { price: 8476.97, size: 1.0 },
                Level { price: 8475.55, size: 1.0 },
            ],
            source: Source::OKX,
        });

        // Then
        assert_eq!((2, 2), orderbook.len());
        assert_eq!(8476.98, orderbook.asks[0].price);
        assert_eq!(8477.0, orderbook.asks[1].price);

        assert_eq!(8476.97, orderbook.bids[0].price);
        assert_eq!(8475.55, orderbook.bids[1].price);
    }

    #[test]
    fn should_keep_the_expected_size() {
        // Given
        let mut orderbook = Orderbook::new(2);
        assert_eq!((0, 0), orderbook.len());

        // When
        orderbook.process(Operation::Update {
            asks: vec![
                Level { price: 8476.98, size: 1.0 },
                Level { price: 8477.0, size: 1.0 },
            ],
            bids: vec![
                Level { price: 8476.97, size: 1.0 },
                Level { price: 8475.55, size: 1.0 },
            ],
            source: Source::OKX,
        });
        assert_eq!((2, 2), orderbook.len());
        orderbook.process(Operation::Update {
            asks: vec![
                Level { price: 8475.98, size: 1.0 },
            ],
            bids: vec![
                Level { price: 8477.97, size: 1.0 },
            ],
            source: Source::OKX,
        });

        // Then
        assert_eq!((2, 2), orderbook.len());
        assert_eq!(8475.98, orderbook.asks[0].price);
        assert_eq!(8476.98, orderbook.asks[1].price);

        assert_eq!(8477.97, orderbook.bids[0].price);
        assert_eq!(8476.97, orderbook.bids[1].price);
    }
}
