use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LineStyle {
    pub start: usize,
    pub end:   usize,
    pub text:  Option<String>,
    pub style: Style
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Style {
    pub fg_color: Option<String>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticStyles {
    pub rev:    u64,
    pub path:   PathBuf,
    pub len:    usize,
    pub styles: Vec<LineStyle>
}

/// main_2.rs
pub fn init_semantic_2() -> SemanticStyles {
    serde_json::from_str(r#"{"rev":1,"path":"D:\\git\\check\\src\\simple-ansi-to-style","len":461,"styles":[{"start":0,"end":2,"text":null,"style":{"fg_color":"keyword"}},{"start":3,"end":7,"text":null,"style":{"fg_color":"function"}},{"start":17,"end":19,"text":null,"style":{"fg_color":"keyword"}},{"start":20,"end":24,"text":null,"style":{"fg_color":"boolean"}},{"start":36,"end":43,"text":null,"style":{"fg_color":"macro"}},{"start":43,"end":44,"text":null,"style":{"fg_color":"macro"}},{"start":45,"end":54,"text":null,"style":{"fg_color":"string"}},{"start":64,"end":68,"text":null,"style":{"fg_color":"keyword"}},{"start":80,"end":87,"text":null,"style":{"fg_color":"macro"}},{"start":87,"end":88,"text":null,"style":{"fg_color":"macro"}},{"start":89,"end":98,"text":null,"style":{"fg_color":"string"}},{"start":113,"end":116,"text":null,"style":{"fg_color":"keyword"}},{"start":117,"end":118,"text":null,"style":{"fg_color":"variable"}},{"start":119,"end":120,"text":null,"style":{"fg_color":"operator"}},{"start":121,"end":122,"text":null,"style":{"fg_color":"struct"}},{"start":128,"end":134,"text":null,"style":{"fg_color":"keyword"}},{"start":135,"end":136,"text":null,"style":{"fg_color":"struct"}},{"start":141,"end":143,"text":null,"style":{"fg_color":"keyword"}},{"start":144,"end":148,"text":null,"style":{"fg_color":"function"}},{"start":158,"end":165,"text":null,"style":{"fg_color":"macro"}},{"start":165,"end":166,"text":null,"style":{"fg_color":"macro"}},{"start":167,"end":169,"text":null,"style":{"fg_color":"string"}},{"start":177,"end":184,"text":null,"style":{"fg_color":"macro"}},{"start":184,"end":185,"text":null,"style":{"fg_color":"macro"}},{"start":186,"end":188,"text":null,"style":{"fg_color":"string"}},{"start":196,"end":203,"text":null,"style":{"fg_color":"macro"}},{"start":203,"end":204,"text":null,"style":{"fg_color":"macro"}},{"start":205,"end":207,"text":null,"style":{"fg_color":"string"}},{"start":215,"end":222,"text":null,"style":{"fg_color":"macro"}},{"start":222,"end":223,"text":null,"style":{"fg_color":"macro"}},{"start":224,"end":226,"text":null,"style":{"fg_color":"string"}},{"start":234,"end":241,"text":null,"style":{"fg_color":"macro"}},{"start":241,"end":242,"text":null,"style":{"fg_color":"macro"}},{"start":243,"end":245,"text":null,"style":{"fg_color":"string"}},{"start":253,"end":260,"text":null,"style":{"fg_color":"macro"}},{"start":260,"end":261,"text":null,"style":{"fg_color":"macro"}},{"start":262,"end":264,"text":null,"style":{"fg_color":"string"}},{"start":272,"end":279,"text":null,"style":{"fg_color":"macro"}},{"start":279,"end":280,"text":null,"style":{"fg_color":"macro"}},{"start":281,"end":283,"text":null,"style":{"fg_color":"string"}},{"start":291,"end":298,"text":null,"style":{"fg_color":"macro"}},{"start":298,"end":299,"text":null,"style":{"fg_color":"macro"}},{"start":300,"end":302,"text":null,"style":{"fg_color":"string"}},{"start":310,"end":317,"text":null,"style":{"fg_color":"macro"}},{"start":317,"end":318,"text":null,"style":{"fg_color":"macro"}},{"start":319,"end":321,"text":null,"style":{"fg_color":"string"}},{"start":329,"end":336,"text":null,"style":{"fg_color":"macro"}},{"start":336,"end":337,"text":null,"style":{"fg_color":"macro"}},{"start":338,"end":340,"text":null,"style":{"fg_color":"string"}},{"start":348,"end":355,"text":null,"style":{"fg_color":"macro"}},{"start":355,"end":356,"text":null,"style":{"fg_color":"macro"}},{"start":357,"end":359,"text":null,"style":{"fg_color":"string"}},{"start":367,"end":374,"text":null,"style":{"fg_color":"macro"}},{"start":374,"end":375,"text":null,"style":{"fg_color":"macro"}},{"start":376,"end":378,"text":null,"style":{"fg_color":"string"}},{"start":386,"end":393,"text":null,"style":{"fg_color":"macro"}},{"start":393,"end":394,"text":null,"style":{"fg_color":"macro"}},{"start":395,"end":397,"text":null,"style":{"fg_color":"string"}},{"start":405,"end":412,"text":null,"style":{"fg_color":"macro"}},{"start":412,"end":413,"text":null,"style":{"fg_color":"macro"}},{"start":414,"end":416,"text":null,"style":{"fg_color":"string"}},{"start":424,"end":431,"text":null,"style":{"fg_color":"macro"}},{"start":431,"end":432,"text":null,"style":{"fg_color":"macro"}},{"start":433,"end":435,"text":null,"style":{"fg_color":"string"}},{"start":443,"end":450,"text":null,"style":{"fg_color":"macro"}},{"start":450,"end":451,"text":null,"style":{"fg_color":"macro"}},{"start":452,"end":454,"text":null,"style":{"fg_color":"string"}}]}"#).unwrap()
}
