//! Pure Puppet/stdlib functions (no scope access): string, array and hash
//! helpers plus `versioncmp` and regex capture support. Anything not listed
//! here is unknown to the evaluator and yields `None`.

use super::{ruby_regex_to_rust, PuppetValue};
use indexmap::IndexMap;

/// Puppet's `versioncmp` (Puppet::Util::Package.versioncmp).
pub(super) fn versioncmp(a: &str, b: &str) -> i64 {
    fn tokens(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '-' || c == '.' {
                out.push(c.to_string());
                i += 1;
            } else if c.is_ascii_digit() {
                let st = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(chars[st..i].iter().collect());
            } else {
                let st = i;
                while i < chars.len()
                    && !matches!(chars[i], '-' | '.')
                    && !chars[i].is_ascii_digit()
                {
                    i += 1;
                }
                out.push(chars[st..i].iter().collect());
            }
        }
        out
    }
    fn ord(o: std::cmp::Ordering) -> i64 {
        match o {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }
    let (ax, bx) = (tokens(a), tokens(b));
    for (x, y) in ax.iter().zip(bx.iter()) {
        if x == y {
            continue;
        }
        if x == "-" {
            return -1;
        }
        if y == "-" {
            return 1;
        }
        if x == "." {
            return -1;
        }
        if y == "." {
            return 1;
        }
        let num = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        if num(x) && num(y) {
            if x.starts_with('0') || y.starts_with('0') {
                return ord(x.to_uppercase().cmp(&y.to_uppercase()));
            }
            let (xi, yi) = (
                x.parse::<u128>().unwrap_or(0),
                y.parse::<u128>().unwrap_or(0),
            );
            return ord(xi.cmp(&yi));
        }
        return ord(x.to_uppercase().cmp(&y.to_uppercase()));
    }
    ord(a.cmp(b))
}

/// Capture groups of the first match of `pattern` in `subject`: index 0 is the
/// whole match; unmatched optional groups are `Undef`. `None` = no match.
pub(super) fn regex_captures(subject: &str, pattern: &str) -> Option<Vec<PuppetValue>> {
    let re = fancy_regex::Regex::new(&ruby_regex_to_rust(pattern)).ok()?;
    let caps = re.captures(subject).ok()??;
    Some(
        (0..caps.len())
            .map(|i| match caps.get(i) {
                Some(m) => PuppetValue::String(m.as_str().to_string()),
                None => PuppetValue::Undef,
            })
            .collect(),
    )
}

fn as_str(v: Option<&PuppetValue>) -> String {
    v.map(|v| v.as_string()).unwrap_or_default()
}

fn to_int(v: Option<&PuppetValue>) -> Option<i64> {
    match v? {
        PuppetValue::Integer(i) => Some(*i),
        PuppetValue::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Dispatch a pure builtin. `None` means the function is not implemented here.
pub(super) fn call(name: &str, args: &[PuppetValue]) -> Option<PuppetValue> {
    use PuppetValue as V;
    let first = args.first();
    Some(match name {
        "versioncmp" => V::Integer(versioncmp(&as_str(first), &as_str(args.get(1)))),
        "split" => {
            let subject = as_str(first);
            let pat = as_str(args.get(1));
            match fancy_regex::Regex::new(&ruby_regex_to_rust(&pat)) {
                Ok(re) => {
                    let mut out = Vec::new();
                    let mut last = 0;
                    for m in re.find_iter(&subject).flatten() {
                        if m.start() == m.end() {
                            continue;
                        }
                        out.push(V::String(subject[last..m.start()].to_string()));
                        last = m.end();
                    }
                    out.push(V::String(subject[last..].to_string()));
                    while out.len() > 1 && matches!(out.last(), Some(V::String(s)) if s.is_empty())
                    {
                        out.pop();
                    }
                    V::Array(out)
                }
                Err(_) => V::Array(vec![V::String(subject)]),
            }
        }
        "match" => {
            let subject = as_str(first);
            match regex_captures(&subject, &as_str(args.get(1))) {
                Some(caps) => V::Array(caps),
                None => V::Undef,
            }
        }
        "delete_at" => match first {
            Some(V::Array(items)) => {
                let mut items = items.clone();
                if let Some(i) = to_int(args.get(1)) {
                    let idx = if i < 0 { items.len() as i64 + i } else { i };
                    if idx >= 0 && (idx as usize) < items.len() {
                        items.remove(idx as usize);
                    }
                }
                V::Array(items)
            }
            _ => V::Undef,
        },
        "flatten" => {
            fn flat(v: &V, out: &mut Vec<V>) {
                match v {
                    V::Array(items) => items.iter().for_each(|i| flat(i, out)),
                    other => out.push(other.clone()),
                }
            }
            let mut out = Vec::new();
            args.iter().for_each(|a| flat(a, &mut out));
            V::Array(out)
        }
        "strip" => V::String(as_str(first).trim().to_string()),
        "lstrip" => V::String(as_str(first).trim_start().to_string()),
        "rstrip" => V::String(as_str(first).trim_end().to_string()),
        "downcase" => V::String(as_str(first).to_lowercase()),
        "upcase" => V::String(as_str(first).to_uppercase()),
        "capitalize" => {
            let s = as_str(first);
            let mut c = s.chars();
            V::String(match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            })
        }
        "keys" => match first {
            Some(V::Hash(m)) => V::Array(m.keys().cloned().map(V::String).collect()),
            _ => V::Array(vec![]),
        },
        "values" => match first {
            Some(V::Hash(m)) => V::Array(m.values().cloned().collect()),
            _ => V::Array(vec![]),
        },
        "has_key" => match first {
            Some(V::Hash(m)) => V::Bool(m.contains_key(&as_str(args.get(1)))),
            _ => V::Bool(false),
        },
        "merge" => {
            let mut out: IndexMap<String, V> = IndexMap::new();
            for a in args {
                if let V::Hash(m) = a {
                    for (k, v) in m {
                        out.insert(k.clone(), v.clone());
                    }
                }
            }
            V::Hash(out)
        }
        "unique" => match first {
            Some(V::Array(items)) => {
                let mut out: Vec<V> = Vec::new();
                for i in items {
                    if !out.contains(i) {
                        out.push(i.clone());
                    }
                }
                V::Array(out)
            }
            _ => V::Undef,
        },
        "sort" => match first {
            Some(V::Array(items)) => {
                let mut items = items.clone();
                items.sort_by_key(|a| a.as_string());
                V::Array(items)
            }
            _ => V::Undef,
        },
        "reverse" => match first {
            Some(V::Array(items)) => V::Array(items.iter().rev().cloned().collect()),
            _ => V::Undef,
        },
        "first" => match first {
            Some(V::Array(items)) => items.first().cloned().unwrap_or(V::Undef),
            _ => V::Undef,
        },
        "last" => match first {
            Some(V::Array(items)) => items.last().cloned().unwrap_or(V::Undef),
            _ => V::Undef,
        },
        "regsubst" if matches!(first, Some(V::Array(_))) => {
            let Some(V::Array(items)) = first else {
                unreachable!()
            };
            V::Array(
                items
                    .iter()
                    .map(|item| {
                        let mut a = args.to_vec();
                        a[0] = item.clone();
                        call("regsubst", &a).unwrap_or(V::Undef)
                    })
                    .collect(),
            )
        }
        "regsubst" => {
            let subject = as_str(first);
            let pat = as_str(args.get(1));
            let repl = as_str(args.get(2));
            let global = as_str(args.get(3)).contains('G');
            match fancy_regex::Regex::new(&ruby_regex_to_rust(&pat)) {
                Ok(re) => {
                    // Puppet uses `\1` back-references; Rust expects `${1}`.
                    let mut r = String::new();
                    let mut it = repl.chars().peekable();
                    while let Some(c) = it.next() {
                        match (c, it.peek().copied()) {
                            ('\\', Some(d)) if d.is_ascii_digit() => {
                                r.push_str(&format!("${{{d}}}"));
                                it.next();
                            }
                            ('$', _) => r.push_str("$$"),
                            _ => r.push(c),
                        }
                    }
                    let out = if global {
                        re.replace_all(&subject, r.as_str()).to_string()
                    } else {
                        re.replace(&subject, r.as_str()).to_string()
                    };
                    V::String(out)
                }
                Err(_) => V::String(subject),
            }
        }
        _ => return None,
    })
}
