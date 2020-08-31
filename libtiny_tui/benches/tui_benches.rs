use criterion::{black_box, criterion_group, criterion_main, Criterion};
use libtiny_tui::msg_area::Line;
use libtiny_tui::trie::Trie;
use std::{fs::File, io::Read};

criterion_group!(
    benches,
    bench_trie_build,
    bench_trie_list_all,
    bench_trie_lookup,
    bench_line_rendered_height,
);
criterion_main!(benches);

fn bench_trie_build(c: &mut Criterion) {
    // Total words: 305,089
    // 117,701,680 ns (0.1 seconds)
    // (before reversing the list: 116,795,268 ns (0.1 seconds))

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open("/usr/share/dict/american-english") {
            Err(_) => {
                println!("Can't open dictionary file, aborting benchmark.");
                return;
            }
            Ok(mut file) => {
                file.read_to_string(&mut contents).unwrap();
                words.extend(contents.lines());
            }
        }
    }

    c.bench_function("trie_build", |b| {
        b.iter(|| {
            let mut trie = Trie::new();
            // Note that we insert the words in reverse order here. Since the
            // dictionary is already sorted, we end up benchmarking the best case.
            // Since that best case is never really happens in practice, the number
            // is practically useless. Worst case is at least giving an upper bound.
            for word in words.iter().rev() {
                trie.insert(word);
            }
            trie
        })
    });
}

/*
#[bench]
fn bench_hashset_build(b : &mut Bencher) {

    // Total words: 305,089
    // 40,292,006 ns (0.04 seconds)

    use std::collections::HashSet;

    let mut contents = String::new();
    let mut words : Vec<&str> = vec![];
    {
        let mut file = File::open("/usr/share/dict/american").unwrap();
        file.read_to_string(&mut contents).unwrap();
        words.extend(contents.lines());
    }

    b.iter(|| {
        let mut set = HashSet::new();
        for word in words.iter() {
            set.insert(word);
        }
        set
    });
}
*/

fn bench_trie_lookup(c: &mut Criterion) {
    // Total:     305,089 words
    // Returning:     235 words
    // 140,717 ns (0.14 ms)

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open("/usr/share/dict/american-english") {
            Err(_) => {
                println!("Can't open dictionary file, aborting benchmark.");
                return;
            }
            Ok(mut file) => {
                file.read_to_string(&mut contents).unwrap();
                words.extend(contents.lines());
            }
        }
    }

    let mut trie = Trie::new();
    for word in words {
        trie.insert(word);
    }
    c.bench_function("trie_lookup", |b| {
        b.iter(|| trie.drop_pfx(&mut "abs".chars()))
    });
}

fn bench_trie_list_all(c: &mut Criterion) {
    // Total:     305,089 words
    // Returning: 305,089 words
    // 205,946,060 ns (0.2 s)

    let mut contents = String::new();
    let mut words: Vec<&str> = vec![];
    {
        match File::open("/usr/share/dict/american-english") {
            Err(_) => {
                println!("Can't open dictionary file, aborting benchmark.");
                return;
            }
            Ok(mut file) => {
                file.read_to_string(&mut contents).unwrap();
                words.extend(contents.lines());
            }
        }
    }

    let mut trie = Trie::new();
    for word in words {
        trie.insert(word);
    }
    c.bench_function("trie_list_all", |b| {
        b.iter(|| trie.drop_pfx(&mut "".chars()))
    });
}

fn bench_line_rendered_height(c: &mut Criterion) {
    // 1160 words, 41,617 ns/iter (+/- 1,595)

    let mut text = String::new();
    {
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();
    }

    let mut line = Line::new();
    line.add_text(&text);
    c.bench_function("line_rendered_height", |b| {
        b.iter(|| {
            line.force_recalculation();
            line.rendered_height(1);
        })
    });
}
