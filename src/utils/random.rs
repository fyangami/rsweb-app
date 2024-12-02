use rand::Rng;

pub fn next_random_alphanumeric(len: usize) -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

const NUM_NUMERIC: usize = 10;
const NUMERIC: [char; NUM_NUMERIC] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

pub fn next_random_numeric(len: usize) -> String {
    (0..len).into_iter().fold(String::new(), |mut s, _| {
        s.push(NUMERIC[rand::thread_rng().gen_range(0..NUM_NUMERIC)]);
        s
    })
}
