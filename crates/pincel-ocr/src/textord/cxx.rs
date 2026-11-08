//! The C++ standard library's `std::sort` and `std::nth_element` as
//! libstdc++ implements them (introsort, introselect).
//!
//! Neither is stable, and Tesseract sorts blobs whose keys tie (equal left
//! edges); the order such ties end up in decides which blob later merges
//! into which, so the port reproduces libstdc++ step by step.

/// `_S_threshold`.
const THRESHOLD: usize = 16;

/// `std::__lg`: the index of the highest set bit.
fn lg(n: usize) -> usize {
    (usize::BITS - 1 - n.leading_zeros()) as usize
}

/// `std::sort` with a strict weak "less than" comparison.
pub(crate) fn sort<T: Clone>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    if v.len() > 1 {
        introsort_loop(v, 0, v.len(), 2 * lg(v.len()), less);
        final_insertion_sort(v, less);
    }
}

/// `std::nth_element`: afterwards `v[nth]` holds the element a full sort
/// would put there.
pub(crate) fn nth_element<T: Clone>(v: &mut [T], nth: usize, less: &impl Fn(&T, &T) -> bool) {
    let (mut first, mut last) = (0usize, v.len());
    if first == last || nth >= last {
        return;
    }
    let mut depth = 2 * lg(last - first);
    while last - first > 3 {
        if depth == 0 {
            heap_select(v, first, nth + 1, last, less);
            v.swap(first, nth);
            return;
        }
        depth -= 1;
        let cut = partition_pivot(v, first, last, less);
        if cut <= nth {
            first = cut;
        } else {
            last = cut;
        }
    }
    insertion_sort(v, first, last, less);
}

fn introsort_loop<T: Clone>(
    v: &mut [T],
    first: usize,
    mut last: usize,
    mut depth: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    while last - first > THRESHOLD {
        if depth == 0 {
            heap_select(v, first, last, last, less);
            sort_heap(v, first, last, less);
            return;
        }
        depth -= 1;
        let cut = partition_pivot(v, first, last, less);
        introsort_loop(v, cut, last, depth, less);
        last = cut;
    }
}

fn partition_pivot<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    let mid = first + (last - first) / 2;
    move_median_to_first(v, first, first + 1, mid, last - 1, less);
    unguarded_partition(v, first + 1, last, first, less)
}

fn move_median_to_first<T>(
    v: &mut [T],
    result: usize,
    a: usize,
    b: usize,
    c: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    let pick = if less(&v[a], &v[b]) {
        if less(&v[b], &v[c]) {
            b
        } else if less(&v[a], &v[c]) {
            c
        } else {
            a
        }
    } else if less(&v[a], &v[c]) {
        a
    } else if less(&v[b], &v[c]) {
        c
    } else {
        b
    };
    v.swap(result, pick);
}

fn unguarded_partition<T>(
    v: &mut [T],
    mut first: usize,
    mut last: usize,
    pivot: usize,
    less: &impl Fn(&T, &T) -> bool,
) -> usize {
    loop {
        while less(&v[first], &v[pivot]) {
            first += 1;
        }
        last -= 1;
        while less(&v[pivot], &v[last]) {
            last -= 1;
        }
        if first >= last {
            return first;
        }
        v.swap(first, last);
        first += 1;
    }
}

fn final_insertion_sort<T: Clone>(v: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    if v.len() > THRESHOLD {
        insertion_sort(v, 0, THRESHOLD, less);
        for i in THRESHOLD..v.len() {
            unguarded_linear_insert(v, i, less);
        }
    } else {
        insertion_sort(v, 0, v.len(), less);
    }
}

fn insertion_sort<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    if first == last {
        return;
    }
    for i in first + 1..last {
        if less(&v[i], &v[first]) {
            v[first..=i].rotate_right(1);
        } else {
            unguarded_linear_insert(v, i, less);
        }
    }
}

fn unguarded_linear_insert<T: Clone>(v: &mut [T], mut last: usize, less: &impl Fn(&T, &T) -> bool) {
    let val = v[last].clone();
    while less(&val, &v[last - 1]) {
        v[last] = v[last - 1].clone();
        last -= 1;
    }
    v[last] = val;
}

fn heap_select<T: Clone>(
    v: &mut [T],
    first: usize,
    middle: usize,
    last: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    make_heap(v, first, middle, less);
    for i in middle..last {
        if less(&v[i], &v[first]) {
            pop_heap(v, first, middle, i, less);
        }
    }
}

fn make_heap<T: Clone>(v: &mut [T], first: usize, last: usize, less: &impl Fn(&T, &T) -> bool) {
    let len = last - first;
    if len < 2 {
        return;
    }
    let mut parent = (len - 2) / 2;
    loop {
        let value = v[first + parent].clone();
        adjust_heap(v, first, parent, len, value, less);
        if parent == 0 {
            return;
        }
        parent -= 1;
    }
}

fn sort_heap<T: Clone>(v: &mut [T], first: usize, mut last: usize, less: &impl Fn(&T, &T) -> bool) {
    while last - first > 1 {
        last -= 1;
        pop_heap(v, first, last, last, less);
    }
}

fn pop_heap<T: Clone>(
    v: &mut [T],
    first: usize,
    last: usize,
    result: usize,
    less: &impl Fn(&T, &T) -> bool,
) {
    let value = v[result].clone();
    v[result] = v[first].clone();
    adjust_heap(v, first, 0, last - first, value, less);
}

fn adjust_heap<T: Clone>(
    v: &mut [T],
    first: usize,
    mut hole: usize,
    len: usize,
    value: T,
    less: &impl Fn(&T, &T) -> bool,
) {
    let top = hole;
    let mut second = hole;
    while second < (len - 1) / 2 {
        second = 2 * (second + 1);
        if less(&v[first + second], &v[first + second - 1]) {
            second -= 1;
        }
        v[first + hole] = v[first + second].clone();
        hole = second;
    }
    if len & 1 == 0 && second == (len - 2) / 2 {
        second = 2 * (second + 1);
        v[first + hole] = v[first + second - 1].clone();
        hole = second - 1;
    }
    // __push_heap
    while hole > top {
        let parent = (hole - 1) / 2;
        if !less(&v[first + parent], &value) {
            break;
        }
        v[first + hole] = v[first + parent].clone();
        hole = parent;
    }
    v[first + hole] = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_orders_many_values() {
        let mut v: Vec<i32> = (0..200).map(|i| (i * 7919) % 101).collect();
        sort(&mut v, &|a, b| a < b);
        assert!(v.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn nth_element_finds_median() {
        let mut v: Vec<i32> = (0..51).map(|i| (i * 37) % 51).collect();
        nth_element(&mut v, 25, &|a, b| a < b);
        assert_eq!(v[25], 25);
    }
}
