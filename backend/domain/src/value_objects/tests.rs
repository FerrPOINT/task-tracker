#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::value_objects::{ArcStr, Email, RichText};

    #[test]
    fn arc_str_conversions() {
        let a = ArcStr::new("hello");
        assert_eq!(a.as_str(), "hello");
        assert_eq!(a.to_string(), "hello");
        assert_eq!(a, ArcStr::from("hello".to_string()));
        assert_eq!(&*a, "hello");

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct Wrapper {
            s: ArcStr,
        }
        let w = Wrapper {
            s: ArcStr::new("x"),
        };
        let json = serde_json::to_string(&w).unwrap();
        assert_eq!(json, r#"{"s":"x"}"#);
        let parsed: Wrapper = serde_json::from_str(&json).unwrap();
        assert_eq!(w, parsed);
    }

    #[test]
    fn rich_text_conversions() {
        let r = RichText::new("body");
        assert_eq!(r.as_str(), "body");
        assert_eq!(r, RichText::from("body"));
        assert_eq!(r, RichText::from("body".to_string()));
    }

    #[test]
    fn email_wrapper() {
        let e = Email::new("a@b.com");
        assert_eq!(e.as_str(), "a@b.com");
        let s: ArcStr = "a@b.com".into();
        assert_eq!(Email::new(s).as_str(), "a@b.com");
    }
}
