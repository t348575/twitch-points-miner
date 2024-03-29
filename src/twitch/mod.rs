pub mod api;
pub mod auth;
pub mod gql;
pub mod ws;

const CLIENT_ID: &str = "ue6666qo983tsx6so1t0vnawi233wa";
const DEVICE_ID: &str = "COF4t3ZVYpc87xfn8Jplkv5UQk8KVXvh";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 7.1; Smart Box C1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";
const FIREFOX_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:84.0) Gecko/20100101 Firefox/84.0";
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";

fn traverse_json<'a>(
    mut value: &'a serde_json::Value,
    mut path: &str,
) -> Option<&'a serde_json::Value> {
    loop {
        let (token, rest) = consume(path);
        path = rest;
        match token {
            Token::Object => {
                if value.is_object() {
                    let (token, rest) = consume(path);
                    path = rest;

                    match token {
                        Token::Name(name) => match value.as_object().unwrap().get(name) {
                            Some(s) => value = s,
                            None => return None,
                        },
                        _ => return None,
                    }
                } else {
                    return None;
                }
            }
            Token::Array(idx) => match idx.parse::<usize>() {
                Ok(idx) => {
                    if value.is_array() {
                        match value.as_array().unwrap().get(idx) {
                            Some(s) => value = s,
                            None => return None,
                        }
                    } else {
                        return None;
                    }
                }
                Err(_) => return None,
            },
            Token::Eos => return Some(value),
            Token::Name(_) => unreachable!(),
        }
    }
}

enum Token<'a> {
    Name(&'a str),
    Object,
    Array(&'a str),
    /// End of stream
    Eos,
}

fn consume<'a>(data: &'a str) -> (Token<'a>, &'a str) {
    let mut started = false;
    for (idx, char) in data.char_indices() {
        match char {
            '.' => {
                if !started {
                    return (Token::Object, &data[idx + 1..]);
                } else {
                    return (Token::Name(&data[..idx]), &data[idx..]);
                }
            }
            '[' => {
                if !started {
                    let array_idx = consume(&data[idx + 1..]);
                    let (_, leftover) = consume(array_idx.1);
                    return (array_idx.0, leftover);
                } else {
                    return (Token::Name(&data[..idx]), &data[idx..]);
                }
            }
            ']' => {
                if !started {
                    return (Token::Eos, &data[idx + 1..]);
                } else {
                    return (Token::Array(&data[..idx]), &data[idx..]);
                }
            }
            _ => {
                if !started {
                    started = true;
                }
            }
        }
    }
    if started {
        (Token::Name(data), "")
    } else {
        (Token::Eos, data)
    }
}

#[cfg(test)]
mod test {
    use crate::twitch::traverse_json;

    #[test]
    fn traverse_regular() {
        let data: serde_json::Value = serde_json::from_str(
            r#"
        {
            "a": {
                "b": {
                    "c": 1
                },
                "d": 2
            }
        }
        "#,
        )
        .unwrap();

        assert_eq!(
            traverse_json(&data, ".a.b.c"),
            Some(&serde_json::Value::Number(1.into()))
        );
        assert_eq!(
            traverse_json(&data, ".a.d"),
            Some(&serde_json::Value::Number(2.into()))
        );
    }

    #[test]
    fn traverse_array() {
        let data: serde_json::Value = serde_json::from_str(
            r#"
        {
            "a": [
                1,
                2
            ],
            "b": {
                "c": [
                    {
                        "d": 3
                    },
                    {
                        "e": 4
                    }
                ]
            }
        }
        "#,
        )
        .unwrap();

        assert_eq!(
            traverse_json(&data, ".a[0]"),
            Some(&serde_json::Value::Number(1.into()))
        );
        assert_eq!(
            traverse_json(&data, ".a[1]"),
            Some(&serde_json::Value::Number(2.into()))
        );

        assert_eq!(
            traverse_json(&data, ".b.c[0].d"),
            Some(&serde_json::Value::Number(3.into()))
        );
        assert_eq!(
            traverse_json(&data, ".b.c[1].e"),
            Some(&serde_json::Value::Number(4.into()))
        );
    }
}
