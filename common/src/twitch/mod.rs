pub mod api;
pub mod auth;
pub mod gql;
pub mod ws;

const CLIENT_ID: &str = "ue6666qo983tsx6so1t0vnawi233wa";
const DEVICE_ID: &str = "COF4t3ZVYpc87xfn8Jplkv5UQk8KVXvh";
const USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 7.1; Smart Box C1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";
const CHROME_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/108.0.0.0 Safari/537.36";

pub fn traverse_json<'a>(
    mut value: &'a mut serde_json::Value,
    mut path: &str,
) -> Option<&'a mut serde_json::Value> {
    loop {
        let (token, rest) = consume(path);
        path = rest;
        match token {
            Token::Object => {
                if value.is_object() {
                    let (token, rest) = consume(path);
                    path = rest;

                    match token {
                        Token::Name(name) => match value.as_object_mut().unwrap().get_mut(name) {
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
                        match value.as_array_mut().unwrap().get_mut(idx) {
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

fn consume(data: &str) -> (Token<'_>, &str) {
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

fn camel_to_snake_case_json(value: &mut serde_json::Value) {
    if value.is_object() {
        let obj = value.as_object_mut().unwrap();
        let mut to_add = Vec::new();
        for item in obj.iter() {
            to_add.push((item.0.clone(), to_snake_case(item.0)));
        }

        to_add.into_iter().for_each(|(old, new)| {
            let mut json = obj.remove(&old).unwrap();
            if json.is_object() || json.is_array() {
                camel_to_snake_case_json(&mut json);
            }
            obj.insert(new, json);
        });
    } else if value.is_array() {
        for item in value.as_array_mut().unwrap() {
            if item.is_object() || item.is_array() {
                camel_to_snake_case_json(item);
            }
        }
    }
}

fn to_snake_case(input: &str) -> String {
    let mut result = String::new();
    let mut prev_char_was_uppercase = true;

    for c in input.chars() {
        if c.is_uppercase() {
            if !prev_char_was_uppercase {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
            prev_char_was_uppercase = true;
        } else {
            result.push(c);
            prev_char_was_uppercase = false;
        }
    }

    result
}

#[cfg(test)]
mod test {
    use crate::twitch::traverse_json;

    #[test]
    fn traverse_regular() {
        let mut data: serde_json::Value = serde_json::from_str(
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
            traverse_json(&mut data, ".a.b.c"),
            Some(&mut serde_json::Value::Number(1.into()))
        );
        assert_eq!(
            traverse_json(&mut data, ".a.d"),
            Some(&mut serde_json::Value::Number(2.into()))
        );
    }

    #[test]
    fn traverse_array() {
        let mut data: serde_json::Value = serde_json::from_str(
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
            traverse_json(&mut data, ".a[0]"),
            Some(&mut serde_json::Value::Number(1.into()))
        );
        assert_eq!(
            traverse_json(&mut data, ".a[1]"),
            Some(&mut serde_json::Value::Number(2.into()))
        );

        assert_eq!(
            traverse_json(&mut data, ".b.c[0].d"),
            Some(&mut serde_json::Value::Number(3.into()))
        );
        assert_eq!(
            traverse_json(&mut data, ".b.c[1].e"),
            Some(&mut serde_json::Value::Number(4.into()))
        );
    }

    #[test]
    fn traverse_base_array() {
        let mut data: serde_json::Value = serde_json::from_str(
            r#"
            [
                1,
                2
            ]
        "#,
        )
        .unwrap();

        assert_eq!(
            traverse_json(&mut data, "[0]"),
            Some(&mut serde_json::Value::Number(1.into()))
        );
        assert_eq!(
            traverse_json(&mut data, "[1]"),
            Some(&mut serde_json::Value::Number(2.into()))
        );
    }
}
