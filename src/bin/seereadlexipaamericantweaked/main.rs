
use feature_refining::readlex::load_readlex_ipa_american_tweaked_table;
use feature_refining::glyphs::{encode, asnames};
use tabled::{Tabled, Table, settings::Style};

#[derive(Tabled)]
pub struct ReadlexIPATweakedEntryDisplay {
  pub latin: String,
  pub ipa: String,
  pub ipa_glyphs: String,
  pub ipa_glyphs_ascii: String,
  pub tweaked_ipa: String,
  pub tweaked_ipa_glyphs: String,
  pub tweaked_ipa_glyphs_ascii: String,
  pub frequency: String
}

fn main() {
  let entries = load_readlex_ipa_american_tweaked_table().unwrap();

  println!("Read {} words", entries.len());

  let top = 100;

  println!("Top {} words:", top);
  println!("{}", Table::new(entries.into_iter().take(top).map(|e| {
    ReadlexIPATweakedEntryDisplay {
      latin: e.latin,
      ipa: e.ipa,
      ipa_glyphs: e.ipa_glyphs.clone().map(|g| encode(&g)).unwrap_or("".to_string()),
      ipa_glyphs_ascii: e.ipa_glyphs.map(|g| asnames(&g)).unwrap_or("".to_string()),
      tweaked_ipa: e.tweaked_ipa.unwrap_or("".to_string()),
      tweaked_ipa_glyphs: e.tweaked_ipa_glyphs.clone().map(|g| encode(&g)).unwrap_or("".to_string()),
      tweaked_ipa_glyphs_ascii: e.tweaked_ipa_glyphs.map(|g| asnames(&g)).unwrap_or("".to_string()),
      frequency: format!("{}", e.frequency)
    }
  })).with(Style::sharp()));
}

