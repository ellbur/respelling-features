// Writes res/frequency-table.json (see frequencies::build_frequency_table),
// which search1 reads.

fn main() {
  feature_refining::frequencies::build_table_and_save_to_file();
  println!("Wrote res/frequency-table.json");
}
