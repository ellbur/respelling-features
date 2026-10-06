
use crate::glyphs::AugGlyph;
use std::collections::HashSet;
use crate::substitutions2 as s2;

pub struct HLSubstitutionList {
  pub substitutions: Vec<HLSubstitution>
}

impl HLSubstitutionList {
  pub fn new(substitutions: Vec<HLSubstitution>) -> HLSubstitutionList {
    let res = HLSubstitutionList {
      substitutions
    };
    
    res.check_back_refs().unwrap();

    res
  }
  
  pub fn check_back_refs(&self) -> Result<(), String> {
    let mut seen_mids = HashSet::new();
    for sub in &self.substitutions {
      if seen_mids.contains(&sub.mid) { return Err("Already contains mid".to_owned()); }
      for g in &sub.anterior.pre_key {
        if let AugGlyph::Synthetic(n) = g {
          if !seen_mids.contains(n) { return Err("Missing back-ref".to_owned()); }
        }
      }
      for g in &sub.anterior.at_key {
        if let AugGlyph::Synthetic(n) = g {
          if !seen_mids.contains(n) { return Err("Missing back-ref".to_owned()); }
        }
      }
      for g in &sub.anterior.post_key {
        if let AugGlyph::Synthetic(n) = g {
          if !seen_mids.contains(n) { return Err("Missing back-ref".to_owned()); }
        }
      }
      for g in &sub.posterior.content {
        if let AugGlyph::Synthetic(n) = g {
          if !seen_mids.contains(n) { return Err("Missing back-ref".to_owned()); }
        }
      }
      seen_mids.insert(sub.mid);
    }

    Ok(())
  }
  
  pub fn next_open_mid(&self) -> u32 {
    let mut upper_bound = 0;
    let mut available: HashSet<u32> = HashSet::new();
    available.insert(0);
    
    for sub in &self.substitutions {
      if sub.mid+1 > upper_bound {
        let new_upper_bound = sub.mid + 1;
        for n in upper_bound+1 .. new_upper_bound+1 {
          available.insert(n);
        }
        upper_bound = new_upper_bound;
      }
      available.remove(&sub.mid);
    }
    
    *available.iter().min().unwrap()
  }
  
  pub fn apply(&self, word: &mut Vec<AugGlyph>) -> bool {
    let mut any_mod = false;
    for s in &self.substitutions {
      if s.apply_anterior(word) { any_mod = true }
    }
    for s in self.substitutions.iter().rev() {
      if s.apply_posterior(word) { any_mod = true }
    }
    any_mod
  }
  
  pub fn apply_copied(&self, word: &Vec<AugGlyph>) -> Option<Vec<AugGlyph>> {
    let mut word = word.clone();
    if self.apply(&mut word) {
      Some(word)
    }
    else {
      None
    }
  }
  
  pub fn apply_copied_always(&self, word: &Vec<AugGlyph>) -> Vec<AugGlyph> {
    let mut word = word.clone();
    self.apply(&mut word);
    word
  }
  
  pub fn apply_posterior_copied(&self, word: &Vec<AugGlyph>) -> Vec<AugGlyph> {
    let mut working = word.clone();
    for sub in self.substitutions.iter().rev() {
      sub.apply_posterior(&mut working);
    }
    working
  }
  
  pub fn low_level(&self) -> s2::SubstitutionList {
    use AugGlyph::*;
    
    let mut anterior_lookups: Vec<s2::Lookup> = vec![];
    let mut working_anterior_lookup: Vec<s2::Substitution> = vec![];
    let mut working_problem_symbols: HashSet<AugGlyph> = HashSet::new();
    let mut working_at_problem_symbols: HashSet<AugGlyph> = HashSet::new();
    let mut has_non_letters: bool = false;
    let mut has_start_or_end: bool = false;
    let mut working_produced_symbols: HashSet<u32> = HashSet::new();
    
    for s in &self.substitutions {
      let here_has_non_letters = s.anterior.at_key.iter().any(|g| !g.is_word_glyph()) || s.posterior.content.iter().any(|g| !g.is_word_glyph());
      
      if 
           s.anterior.at_key.iter().any(|g| working_problem_symbols.contains(&g))
        || s.anterior.pre_key.iter().any(|g| working_at_problem_symbols.contains(&g))
        || s.anterior.post_key.iter().any(|g| working_at_problem_symbols.contains(&g))
        || s.anterior.at_key.iter().any(|g| match g { Real(_) => false, Synthetic(n) => working_produced_symbols.contains(&n) })
        || s.anterior.pre_key.iter().any(|g| match g { Real(_) => false, Synthetic(n) => working_produced_symbols.contains(&n) })
        || s.anterior.post_key.iter().any(|g| match g { Real(_) => false, Synthetic(n) => working_produced_symbols.contains(&n) })
        || (has_non_letters && (s.anterior.at_start || s.anterior.at_end))
        || (has_start_or_end && here_has_non_letters)
      {
        if !working_anterior_lookup.is_empty() {
          anterior_lookups.push(s2::Lookup { substitutions: working_anterior_lookup });
          working_anterior_lookup = vec![];
        }
        working_problem_symbols.clear();
        working_at_problem_symbols.clear();
        has_non_letters = false;
        has_start_or_end = false;
        working_produced_symbols.clear();
      }
      
      working_anterior_lookup.extend(s.anterior_low_level());
      
      working_problem_symbols.extend(s.anterior.at_key.clone());
      working_problem_symbols.extend(s.anterior.pre_key.clone());
      working_problem_symbols.extend(s.anterior.post_key.clone());
      working_at_problem_symbols.extend(s.anterior.at_key.clone());
      has_non_letters = has_non_letters || here_has_non_letters;
      has_start_or_end = has_start_or_end || s.anterior.at_start || s.anterior.at_end;
      working_produced_symbols.insert(s.mid);
    }
    if !working_anterior_lookup.is_empty() {
      anterior_lookups.push(s2::Lookup { substitutions: working_anterior_lookup });
    }
    
    let mut posterior_lookups: Vec<s2::Lookup> = vec![];
    let mut working_posterior_lookup: Vec<s2::Substitution> = vec![];
    let mut working_problem_symbols: HashSet<u32> = HashSet::new();
    
    for s in self.substitutions.iter().rev() {
      if working_problem_symbols.contains(&s.mid) {
        if !working_posterior_lookup.is_empty() {
          posterior_lookups.push(s2::Lookup { substitutions: working_posterior_lookup });
          working_posterior_lookup = vec![];
        }
        working_problem_symbols.clear();
      }
      
      working_posterior_lookup.push(s.posterior_low_level());
      
      for g in &s.posterior.content {
        match g {
          Real(_) => (),
          Synthetic(n) => { working_problem_symbols.insert(*n); }
        }
      }
    }
    if !working_posterior_lookup.is_empty() {
      posterior_lookups.push(s2::Lookup { substitutions: working_posterior_lookup });
    }
    
    let mut lookups = anterior_lookups;
    lookups.extend(posterior_lookups);
    
    s2::SubstitutionList {
      lookups
    }
  }
  
  pub fn decode(text: &str) -> Result<HLSubstitutionList, String> {
    let substitutions: Result<Vec<HLSubstitution>, String> = text.split("\n").map(|line| {
      let line = line.trim();
      if line.is_empty() {
        None.into_iter()
      }
      else {
        Some(HLSubstitution::decode(line)).into_iter()
      }
    }).flatten().collect();
    let substitutions = substitutions?;
    
    let hlist = HLSubstitutionList {
      substitutions
    };
    
    hlist.check_back_refs()?;
    
    Ok(hlist)
  }
  
  pub fn set_1() -> HLSubstitutionList {
    HLSubstitutionList::decode("
      [th]→0→ϑ
      [i]→1→ɪ
      [a]→2→æ
      [e]→3→ʌ
      [{3}r]→4→ʳ
      ^[of]→5→{3}v
      [ng]→6→ŋ
      [s]$→7→z
      [c]→8→k
      [ou]→9→u
      [{3}{2}]→10→ɩ
      [to]→11→t{9}
      [y]$→12→{10}
      [o]r→13→ɔ
      [{2}]$→14→a
      [{8}h]→15→ʧ
      [u]→16→{3}
      [{3}{3}]→17→{12}
      [{8}{3}]→18→s
      [o]n→19→{16}
      [ll]→20→l
      [sh]→21→ʃ
      [{1}]$→22→i
      [t{1}]{19}→23→{21}
      [r{3}]→24→r
      [{2}{12}]→25→ϵ
      [{1}gh]→26→{22}
      [{2}{1}]→27→{25}
      [ow]→28→o
      [v{3}]→29→v
      [wh]→30→w
      [s{3}]$→31→{7}
      [l{3}]$→32→{19}{20}
      [n{3}]→33→n
      [o]m→34→{19}
      [{3}{12}]→35→{27}
      [{9}l]d$→36→ɜ
      [{0}]$→37→θ
      [oo]→38→{36}
      [{1}{3}]→39→{17}
      [x]→40→{8}{18}
      [m{3}]→41→m
      [{4}{3}]$→42→ε{24}
      b[{3}]→43→{39}
      [{2}l]→44→{32}
      t[{7}]→45→{18}
      [{2}r]→46→ɑ{24}
      w[{2}]→47→{34}
      ^[{19}]n$→48→ɑ
      [k{3}]→49→{8}
      [ot]$→50→{48}t
      [h{3}]$→51→h{43}
      w[{13}r]→52→{4}
      ^[{3}]→53→ε
      [{9}]t→54→ʊ
      [{2}t{3}]→55→{35}t
      [{16}r]→56→{52}
      {0}[{1}{7}]→57→{1}{45}
      [{3}d]→58→d
      [{0}{1}]→59→{37}{1}
      [{9}gh]→60→{9}
      [{30}o]→61→h{60}
      w[{3}]→62→{43}
      [{2}{24}]→63→{46}
      ^d[o]→64→{60}
      [g{3}]$→65→j
      [{3}{1}r]→66→{42}
      [w{42}]→67→{30}{56}
      [{2}]{20}→68→{13}
      [{2}y]→69→{35}
      [{8}k]→70→{49}
      [q{16}]→71→{70}{30}
      [{9}r]→72→{68}{24}
      [{3}t]→73→{53}t
      ^[{24}]→74→{24}{1}
      [{30}{2}]→75→{30}{47}
      [{2}]b→76→{47}
      [{1}r]→77→{56}
      [{3}o]→78→{62}
      [{3}{8}]→79→{53}{70}
      [s{7}]→80→{45}
      [t{16}{24}]→81→{15}{77}
      [ss]→82→{80}
      ^[b{12}]→83→b{26}
      [pp]→84→p
      {21}[{3}]→85→{78}
      [{3}]n→86→{53}
      [{1}d{3}]→87→{26}{58}
      [tt]→88→t
      [{2}]{23}→89→{69}
      ^[{16}s]→90→y{64}{31}
      [{41}n]→91→{41}{76}{33}
      ^[m{12}]→92→{41}{26}
      ^[{41}]→93→{41}{85}
      ^[{2}n]$→94→{76}{33}
      [nk]→95→{6}{70}
      [{1}{49}]→96→{26}{70}
      [o]{0}{4}→97→{76}
      ^[{19}{33}]→98→{75}{33}
      [{5}]f→99→{68}
      [{2}]{49}→100→{89}
      [{43}{8}{2}]→101→{1}{70}
      [w]$→102→{64}
      [t]{58}→103→{88}{1}
      [{9}n]→104→{54}{33}
      [kn]→105→{33}
      [{2}{16}]→106→{99}
      [ph]→107→f
      [{16}{3}]→108→{102}
      [{1}]{41}→109→{26}
      [{1}t{3}]→110→{109}{88}
      ^[{2}m{4}]→111→{97}{41}{66}
      [{16}n{1}]→112→y{108}{105}{97}
      [{10}r]→113→{1}{24}
      [{53}{29}]→114→{85}{29}{1}
      [two]→115→{11}
      [ff]→116→{107}
      [{2}{41}]→117→{100}{41}
      m[r]→118→{57}{88}{77}
      ^[d{3}]→119→{58}{1}
      [{11}r]→120→{88}{77}
      h[{28}]→121→{54}
      [{2}{6}{3}]→122→{100}{105}{65}
      [{3}{7}]→123→{31}
      [s{27}]→124→{82}{86}
      [{53}{40}]→125→{1}{40}
      ^v[{4}]{12}→126→{66}
      [p{24}s]→127→{84}{24}{86}{123}
      ^[{44}]→128→{106}{20}
      [{2}g{27}]→129→{97}g{86}
      [{1}f{3}]→130→{109}{116}
      [{3}l]→131→{86}{20}
      b[{17}]→132→{1}
      [{2}]n{12}→133→{86}
      [{0}]r→134→{37}
      [{44}]k→135→{106}
      {0}[{3}m]→136→{133}{41}
      {16}[{7}]→137→{82}
      [{1}{32}]→138→{109}{20}
      [{2}ny]→139→{133}{105}{85}
      [{8}]{1}→140→{137}
      [{2}d{3}]$→141→{100}{58}
      ^[{8}{14}]→142→{140}{85}{100}
      [{9}{7}]→143→{97}{140}
      [o{6}]→144→{135}{6}
      [o{1}]n→145→ꭢ
      ^n[{28}]→146→{121}
      [t{3}]→147→{88}{133}
      [{2}]{18}→148→{100}
      [{4}y]→149→{24}{85}
      ^[{72}]→150→{146}{24}
      [{140}{1}]{44}→151→{23}
      [{10}h]→152→{2}
      [{62}{20}]→153→{131}
      [mm]→154→{41}
      [h{42}]→155→{51}{24}
      [{47}n]→156→{48}{105}
      [{47}y]→157→{148}
      [{11}o]→158→{115}
      [{15}{38}]→159→{70}{108}
      ^l[{3}]→160→{133}
      [{1}{19}]→161→{97}
      [{1}nd]$→162→{109}{105}{58}
      [{46}{104}]→163→{77}{104}
      [{1}{24}]→164→{109}{77}
      [ob]→165→{48}b
      [{56}{1}]→166→{38}{74}
      [{3}{31}]→167→{85}{123}
      [{1}{14}]→168→{85}{161}
      [{2}{102}]→169→{135}
      [o]d→170→{48}
      [{16}l{46}]→171→y{44}{77}
      [{19}nl]→172→{28}{105}{20}
      [{16}s{1}{33}{80}]→173→{132}{123}{105}{57}
      [{53}n{60}]→174→{132}{105}{161}{116}
      ^[{113}]→175→{77}
      [{49}d]→176→{70}{88}
      ^[{16}{31}]→177→{90}
      [{68}{20}{12}]→178→{20}{85}
      [{1}]{33}→179→{109}
      [{5}t{86}n]→180→{169}{116}{94}
      [{125}]{2}→181→{132}g{123}
      [{46}r]→182→{126}
      [{18}]{7}→183→{140}{132}
      {1}{8}[{2}]→184→{161}
      d[{58}]→185→{132}{58}
      [dd]→186→{58}
      [{9}{31}]→187→{146}{140}
      ^[y{123}]→188→y{160}{140}
      [g{86}]→189→{65}{160}
      [s{16}]{24}→190→{151}{38}
      [{147}]$→191→{88}
      [h{10}]→192→h{160}
      [{0}{60}]t→193→{134}{169}
      [{2}{31}]→194→{157}{140}
      k[{7}]→195→{140}
      [o{34}]→196→{108}
      [oh]→197→{28}
      [h{72}]→198→{146}{175}
      ^s[{120}]→199→{191}{72}
      [{3}s{23}]→200→{160}{195}{15}
      [{64}]{123}→201→{184}
      [nn]→202→{105}
      [{8}{39}]→203→{179}{201}
      [{74}s{113}]→204→{149}{195}{175}
      [oy]→205→{145}
      {53}[d{16}]→206→{65}y{196}
      [t{16}d{86}n]→207→{158}{186}{94}
      [{4}{39}]→208→{113}{85}
      d[{28}n]→209→{104}
      [{1}{2}]→210→{201}
      [wr]→211→{24}
      [{18}n]→212→{124}{202}
      ^[s{3}]→213→{124}
      [{46}]{12}→214→{182}
      p[{7}]→215→{195}
      w[{46}]→216→{72}
      [o{2}]→217→{197}
      [r{10}]{20}→218→{74}
      [{8}{4}]→219→{215}{175}
      [{16}m{2}n]→220→y{196}{91}
      [{30}{12}]→221→{30}{179}
      [{27}r]→222→{214}
      [{1}d{86}]→223→{210}{186}{160}
      [{33}v]→224→{202}{160}{29}
      [ys{147}]→225→{57}{191}{210}
      [{16}{12}]→226→{179}
      [{2}]{65}→227→{132}
      [s{69}]{7}→228→{213}
      [{16}{24}]→229→y{175}
      [{164}{8}]→230→{113}{79}
      [{4}{1}]→231→{208}
      [lo]p→232→{153}{210}
      [o{29}]→233→{196}{29}
      d[r]$→234→{170}{70}{120}
      [o{82}{1}]→235→{170}{215}{210}
      [{63}n]→236→{222}{94}
      [{4}{86}n]→237→{175}{94}
      [{11}w]→238→{191}
      [{10}{190}{24}]→239→{160}ʒ{175}
      ^l[{113}]→240→{175}
      [{53}y]→241→{226}
      [{82}]{161}→242→{151}
      [{2}{8}]{8}→243→{210}
      [{1}{13}r]→244→{229}
      [o{19}]→245→{196}
      [{43}l]→246→{227}{20}
      [s]{161}→247→ʒ
      [{106}gh]→248→{169}
      [{55}{7}]→249→{55}{215}
      m[{16}s]{1}→250→{177}
      ^[{2}]{84}→251→{243}
      [{1}d{10}]→252→{87}{168}
      [{46}{10}]→253→{222}{168}
      h[{34}]→254→{217}
      [o{12}]→255→{205}
      [g]{123}→256→{65}{227}
      [g{3}]→257→{189}
      [{16}{1}]→258→{227}
      [{2}w]→259→{251}{30}
      [w{7}]→260→{250}
      [l{233}]→261→{20}{5}
      l[{16}]→262→{245}
      [{231}]{6}→263→{240}{258}
      [{8}]{12}→264→{215}
      [t{15}]→265→{15}
      [{73}t]→266→{73}
      [{13}rr]→267→{63}
      [olog]→268→{170}{20}{251}{65}
      [p{3}]$→269→{84}
      [{11}]p→270→{238}{170}
      [{11}n]→271→{238}{94}
      {0}[{60}]→272→{254}
      [{11}{257}]→273→{238}{129}
      [{165}{2}m{14}]→274→{272}b{170}{154}{251} 
      [{2}{32}]→275→{157}{20}
      {18}[d]→276→{238}
      [h{88}p]→277→{157}{265}{276}{85}{276}{85}{269}{85}
      d[{79}{1}]→278→{57}{258}
      [{77}{2}]→279→{113}{170}
      gr[{10}t]→280→{55}
      {8}[o]→281→{251}
      [{79}{166}]→282→{101}y{38}{211}{281}
      [o{1}{18}]→283→{255}{264}
      [{2}k]→284→{157}{70}
      [{119}s{1}g]→285→{119}{123}{241}
      [{1}l{12}]→286→{44}{85}
      [{74}{212}]→287→{149}{264}{94}
      [y{1}]→288→{241}{258}
      [{43}]t→289→{258}
      ^[{91}]→290→{154}{160}{202}
      {8}[{63}]→291→{222}
      [{34}m{91}]→292→{170}{290}
      [{2}]n{97}→293→{281}
      [{61}{32}]→294→h{272}{20}
      [n{31}]→295→{202}{264}
      [os]{1}→296→{293}{123}
      [{53}{56}o]→297→y{38}{211}{293}
      [{34}m{2}n]→298→{38}{91}
      [t{1}{44}]→299→{242}{44}
      [{68}{20}{28}]→300→{44}{146}
      [{13}rm{89}]→301→{240}{154}{157}
      [ol{1}]→302→{44}{289}
      [{0}{7}]→303→{134}{264}
      [{20}]{161}→304→{20}y
      f[{3}]→305→y
      {10}[s]{19}→306→{123}
      [{114}nt]→307→{289}{29}{160}{202}{276}
      ^[{2}]{88}→308→{293}
      [{1}v{1}d{16}]→309→{5}{289}{65}{259}
      [{43}{88}]→310→{266}
      [p{16}]t→311→{269}{38}
      [{0}{2}{95}]→312→{134}{152}{95}
      [{43}g]→313→{289}g
      [o{18}]→314→{170}{228}
      [{1}z{3}]→315→{241}{306}
      [{33}{40}]→316→{202}{160}{40}
      ^[{2}g]→317→{308}g
      [{2}m{144}]→318→{308}{154}{308}{6}
      r[{39}]n→319→{160}
      [{8}{18}]→320→{40}{319}
      [{44}]f→321→{152}
      [gov]→322→g{5}
      [p{28}]→323→{269}{146}
      [{53}v{4}{12}]→324→{319}{29}{149}
      [{15}{2}{6}]→325→{265}{122}
      [p{24}]→326→{269}{211}{319}
      [{24}n]→327→{211}{94}
      [{113}d]→328→{240}{186}
      [{49}{7}]→329→{40}
      [{53}{1}]→330→{85}
      ^[{7}]→331→{319}{264}
      {1}[t{16}]→332→{158}
      [l{259}]→333→{20}{248}
      ^[{2}]{82}→334→{308}
      s[{123}]→335→{289}{306}
      [{43}s]→336→{331}
      [{55}d]→337→{280}{185}
      [{11}]l→338→{276}{272}
      [{104}]tr→339→{94}
      [{10}d{12}]→340→{319}{186}{330}
      [{0}{54}]→341→{134}{146}
      [w{34}]→342→{30}{289}
      [{2}j{13}r]$→343→{157}{65}{240}
      n[{14}]→344→{170}
      [o]{80}→345→{248}
      [{16}{81}]→346→{305}{262}{81}
      [{16}]{20}→347→{38}
      [{2}t{39}n]→348→{157}{242}{339}
      t[{2}n]t→349→{339}
      [{2}t{14}]→350→{280}{334}
      [{1}l{46}]→351→{44}{240}
      [{43}]h→352→{289}
      [{62}]n→353→{319}
      [{93}m]→354→{154}{136}
      [{125}]{1}→355→{181}
      [{44}{108}]→356→{321}{304}{262}
      [o{84}{13}rt{112}]→357→{344}{269}{240}{332}{202}{334}
      [{55}g]→358→{321}{276}{334}{65}
      [{43}]{8}→359→{352}
      [{16}y{7}]→360→{315}
      {4}[{3}]→361→{359}
      [{281}{20}{3}]→362→{344}{20}{361}
      [{82}{108}]→363→{242}{262}
      [{1}{55}]→364→{168}{276}
      r[ov]→365→{5}
      [{79}{1}]→366→{353}{242}
      [{366}f]→367→{278}{116}
      ^[{102}]→368→{186}{334}b{44}{305}{262}
      [{0}{9}s{2}n]→369→{341}{306}{349}
      [{74}{127}]→370→{211}{353}{269}{218}{306} 
      [{46}{1}]→371→{291}{361}
      [of{305}]→372→{334}{116}{353}
      [{72}{31}]→373→{216}{264}
      [{2}{131}]→374→{44}
      [{112}t]→375→{305}{262}{202}{361}{276}
      b[{2}s]→376→{194}
      [y]s→377→{361}
      [{2}v{13}r]→378→{157}{29}{240}
      {1}[g{1}]→379→{65}{334}
      [s]{58}→380→{306}
      [o{59}]→381→{334}{59}
      [{59}r]→382→{134}{240}
      [{86}n]{18}→383→{349}
      [{15}r]→384→{70}{211}
      [{2}{189}n]→385→{157}{65}{383}
      [{4}r{13}r]→386→{291}{240}
      [{9}{6}]→387→{334}{6}
      [{302}{264}]→388→{344}{20}{143}
      [{47}]t→389→{345}
      {8}[{144}]→390→{344}{6}g
      [{158}]k→391→{276}{347}
      [{82}{58}]→392→{264}{276}
      [{15}{46}{2}]→393→{70}{291}{334}
      [{39}d]→394→{87}
      [{47}]{265}→395→{344}
      [{119}mo]→396→{186}{136}{334}
      [{39}{102}]→397→{305}{262}
      [{10}{16}]→398→{397}
      [o{203}]→399→{143}{203}
    ").unwrap()
  }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct HLSubstitution {
  pub anterior: Anterior,
  pub mid: u32,
  pub posterior: Posterior
}

impl HLSubstitution {
  pub fn anterior_low_level(&self) -> Vec<s2::Substitution> {
    self.anterior.low_level(self.mid)
  }
  
  pub fn posterior_low_level(&self) -> s2::Substitution {
    self.posterior.low_level(self.mid)
  }
  
  pub fn apply_anterior(&self, word: &mut Vec<AugGlyph>) -> bool {
    self.anterior.apply(word, self.mid)
  }
  
  pub fn apply_posterior(&self, word: &mut Vec<AugGlyph>) -> bool {
    self.posterior.apply(word, self.mid)
  }
  
  pub fn deapply_posterior(&self, word: &mut Vec<AugGlyph>) -> bool {
    self.posterior.deapply(word, self.mid)
  }
  
  pub fn apply(&self, word: &mut Vec<AugGlyph>) -> bool {
    self.apply_anterior(word) && self.apply_posterior(word)
  }
  
  pub fn apply_copied(&self, word: &Vec<AugGlyph>) -> Option<Vec<AugGlyph>> {
    let mut word = word.clone();
    if self.apply_anterior(&mut word) && self.apply_posterior(&mut word) {
      Some(word)
    }
    else {
      None
    }
  }
  
  pub fn encode(&self) -> String {
    use crate::glyphs::aug_encode;
    format!("{}→{}→{}",
      self.anterior.encode(),
      self.mid,
      aug_encode(&self.posterior.content)
    )
  }
  
  pub fn decode(text: &str) -> Result<HLSubstitution, String> {
    if let [anterior_str, mid_str, posterior_str] = text.split("→").collect::<Vec<_>>()[..] {
      Ok(HLSubstitution {
        anterior: Anterior::decode(anterior_str)?,
        mid: mid_str.parse().map_err(|e| format!("{:?}", e))?,
        posterior: Posterior {
          content: crate::glyphs::aug_decode(posterior_str)
        }
      })
    }
    else {
      Err("Doesn't have 3 parts separated by →".to_owned())
    }
  }
}

impl std::fmt::Debug for HLSubstitution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
    f.write_str(&self.encode())
  }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anterior {
  pub pre_key: Vec<AugGlyph>,
  pub at_key: Vec<AugGlyph>,
  pub post_key: Vec<AugGlyph>,
  pub at_start: bool,
  pub at_end: bool,
  // Anti-anchors (written ~): the glyph before pre_key (after post_key) must
  // be a word glyph, so the key can't be at the start (end) of a word.
  pub not_at_start: bool,
  pub not_at_end: bool,
}

impl Anterior {
  pub fn apply(&self, word: &mut Vec<AugGlyph>, mid: u32) -> bool {
    let mut pos: usize = 0;
    let mut any_mod = false;
    while pos < word.len() {
      if !(
           (pos < self.pre_key.len())
        || (pos + self.at_key.len() + self.post_key.len() > word.len())
        || (self.at_start && pos > self.pre_key.len() && word[pos-self.pre_key.len()-1].is_word_glyph())
        || (self.at_end && pos + self.at_key.len() + self.post_key.len() < word.len() && word[pos+self.at_key.len()+self.post_key.len()].is_word_glyph())
        || (self.not_at_start && !(pos > self.pre_key.len() && word[pos-self.pre_key.len()-1].is_word_glyph()))
        || (self.not_at_end && !(pos + self.at_key.len() + self.post_key.len() < word.len() && word[pos+self.at_key.len()+self.post_key.len()].is_word_glyph()))
        || (&self.pre_key != &word[pos - self.pre_key.len() .. pos])
        || (&self.at_key != &word[pos .. pos + self.at_key.len()])
        || (&self.post_key != &word[pos + self.at_key.len() .. pos + self.at_key.len() + self.post_key.len()])
      ) {
        let working_s1 = pos;
        let working_s2 = pos + self.at_key.len();
        word.splice(working_s1 .. working_s2, vec![AugGlyph::Synthetic(mid)]);
        any_mod = true;
      }
      
      pos += 1;
    }
    any_mod
  }
  
  pub fn decode(anterior_str: &str) -> Result<Anterior, String> {
    let (at_start, anterior_str) =
      if let [first, rest] = anterior_str.split("^").collect::<Vec<_>>()[..] {
        if first != "" {
          return Err("^ not at start".to_owned());
        }
        (true, rest)
      }
      else {
        (false, anterior_str)
      };

    let (at_end, anterior_str) =
      if let [rest, last] = anterior_str.split("$").collect::<Vec<_>>()[..] {
        if last != "" {
          return Err("$ not at end".to_owned());
        }
        (true, rest)
      }
      else {
        (false, anterior_str)
      };

    let (not_at_start, anterior_str) = match anterior_str.strip_prefix('~') {
      Some(rest) => (true, rest),
      None => (false, anterior_str)
    };
    let (not_at_end, anterior_str) = match anterior_str.strip_suffix('~') {
      Some(rest) => (true, rest),
      None => (false, anterior_str)
    };

    if let [pre_str, rest] = anterior_str.split("[").collect::<Vec<_>>()[..] {
      if let [at_str, post_str] = rest.split("]").collect::<Vec<_>>()[..] {
        Ok(Anterior {
          at_start,
          at_end,
          not_at_start,
          not_at_end,
          pre_key: crate::glyphs::aug_decode(pre_str),
          at_key: crate::glyphs::aug_decode(at_str),
          post_key: crate::glyphs::aug_decode(post_str)
        })
      }
      else {
        Err("No ]".to_owned())
      }
    }
    else {
      Err("No [".to_owned())
    }
  }
  
  pub fn low_level(&self, mid: u32) -> Vec<s2::Substitution> {
    let mut working = vec![];
    
    // Anti-anchors are an extra "any word glyph" of context.
    let mut el_pre_key: Vec<s2::KeyElem> = self.pre_key.iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    let mut el_post_key: Vec<s2::KeyElem> = self.post_key.iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
    if self.not_at_start {
      el_pre_key.insert(0, s2::KeyElem::AnyLetter);
    }
    if self.not_at_end {
      el_post_key.push(s2::KeyElem::AnyLetter);
    }
    
    if self.at_start {
      let mut start_pre_key = vec![s2::KeyElem::AnyLetter];
      start_pre_key.extend(el_pre_key.clone());
      let mut end_post_key: Vec<s2::KeyElem> = self.at_key[1 ..].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
      end_post_key.extend(el_post_key.clone());
      working.push(s2::Substitution {
        pre_key: start_pre_key,
        at_key: vec![self.at_key[0]],
        post_key: end_post_key,
        sub_content: s2::SubContent::Ignore
      });
    }
    
    if self.at_end {
      let mut end_post_key: Vec<s2::KeyElem> = self.at_key[1 ..].iter().map(|g| s2::KeyElem::Glyph(*g)).collect();
      end_post_key.extend(el_post_key.clone());
      end_post_key.push(s2::KeyElem::AnyLetter);
      working.push(s2::Substitution {
        pre_key: el_pre_key.clone(),
        at_key: vec![self.at_key[0]],
        post_key: end_post_key,
        sub_content: s2::SubContent::Ignore
      });
    }
    
    working.push(s2::Substitution {
      pre_key: el_pre_key,
      at_key: self.at_key.clone(),
      post_key: el_post_key,
      sub_content: s2::SubContent::Sub(vec![AugGlyph::Synthetic(mid)])
    });
    
    working
  }
  
  pub fn encode(&self) -> String {
    use crate::glyphs::aug_encode;
    format!("{}{}[{}]{}{}",
      if self.at_start { "^" } else if self.not_at_start { "~" } else { "" },
      aug_encode(&self.pre_key),
      aug_encode(&self.at_key),
      aug_encode(&self.post_key),
      if self.at_end { "$" } else if self.not_at_end { "~" } else { "" },
    )
  }
}

#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Posterior {
  pub content: Vec<AugGlyph>
}

impl Posterior {
  pub fn deapply(&self, word: &mut Vec<AugGlyph>, mid: u32) -> bool {
    let k = self.content.len();
    let mut i = 0;
    let mut any_mod = false;
    while i + k < word.len() + 1 {
      let left = &word[i .. i + k];
      let right = &self.content;
      if left == right {
        word.splice(i .. i + k, [AugGlyph::Synthetic(mid)]);
        any_mod = true;
      }
      i += 1;
    }
    any_mod
  }
  
  pub fn apply(&self, word: &mut Vec<AugGlyph>, mid: u32) -> bool {
    let mut i = 0;
    let mut any_mod = false;
    while i < word.len() {
      if word[i] == AugGlyph::Synthetic(mid) {
        word.splice(i .. i + 1, self.content.clone());
        any_mod = true;
        i += self.content.len();
      }
      else {
        i += 1;
      }
    }
    any_mod
  }

  pub fn low_level(&self, mid: u32) -> s2::Substitution {
    s2::Substitution {
      pre_key: vec![],
      at_key: vec![AugGlyph::Synthetic(mid)],
      post_key: vec![],
      sub_content: s2::SubContent::Sub(self.content.clone())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::glyphs::{aug_decode, aug_encode};
  
  #[test]
  fn test_deapply_posterior() {
    let word = aug_decode("kæt");
    let posterior = Posterior {
      content: aug_decode("kæ")
    };
    let mut deapplied = word.clone();
    assert_eq!(posterior.deapply(&mut deapplied, 0), true);
    assert_eq!(deapplied, aug_decode("{0}t"));
  }
  
  #[test]
  fn test_deapply_posterior_2() {
    for (word_str, posterior_str, should_str) in [
      ("kæt", "kæ", "{0}t"),
      ("apple", "p", "a{0}{0}le"),
      ("aadam", "aa", "{0}dam"),
      ("aaadam", "aa", "{0}adam"),
      ("aaaadam", "aa", "{0}{0}dam"),
    ] {
      let word = aug_decode(word_str);
      let posterior = Posterior {
        content: aug_decode(posterior_str)
      };
      let mut deapplied = word.clone();
      assert_eq!(posterior.deapply(&mut deapplied, 0), true);
      assert_eq!(aug_encode(&deapplied), should_str);
    }
  }
  
  #[test]
  fn test_apply_anterior_1() {
    let anterior = Anterior::decode("[ca]").unwrap();
    
    let word = aug_decode("cat");
    
    let mut applied = word.clone();
    assert_eq!(anterior.apply(&mut applied, 0), true);
    assert_eq!(applied, aug_decode("{0}t"));
  }
  
  #[test]
  fn test_apply_anterior_2() {
    for (word_str, anterior_str, should_str, should_match) in [
      ("cat", "[ca]", "{0}t", true),
      ("cat", "[ca]$", "cat", false),
      ("cat", "^[ca]", "{0}t", true),
      ("abc", "a[b]c", "a{0}c", true),
      ("aa", "a[a]", "a{0}", true),
      ("aaa", "a[a]", "a{0}a", true),
      ("aaaa", "a[a]", "a{0}a{0}", true),
      ("cat", "^[a]", "cat", false),
    ] {
      println!("{} {} {} {}", word_str, anterior_str, should_str, should_match);
      
      let anterior = Anterior::decode(anterior_str).unwrap();
      
      let word = aug_decode(word_str);
      
      let mut applied = word.clone();
      assert_eq!(anterior.apply(&mut applied, 0), should_match);
      assert_eq!(aug_encode(&applied), should_str);
      
      println!("");
    }
  }
  
  #[test]
  fn test_apply_anterior_3() {
    let anterior = Anterior::decode("^[-]").unwrap();
    
    let word = aug_decode("a-ʃb");
    
    let mut applied = word.clone();
    assert_eq!(anterior.apply(&mut applied, 0), false);
  }
  
  #[test]
  fn test_encoding_1() {
    for str in [
      "[a]→3→b",
      "^[a]→3→b",
      "^[a]$→3→b",
      "[a]$→3→bgq",
      "[a]{7}$→3→bgq",
    ] {
      println!("{}", str);
      assert_eq!(&HLSubstitution::encode(&HLSubstitution::decode(str).unwrap()), str);
    }
  }
  
  #[test]
  fn posterior_low_level_test_1() {
    for (posterior_str, should_str) in [
      ("a", "sub syn0' by a;"),
      ("ab", "sub syn0' by a b;"),
    ] {
      let posterior = Posterior { content: aug_decode(posterior_str) };
      let low_level = posterior.low_level(0);
      assert_eq!(crate::fea_parsing::render_fea_sub(&low_level), should_str);
    }
  }
  
  #[test]
  fn anterior_low_level_test_1() {
    for (anterior_str, should_str) in [
      ("[a]", "sub a' by syn0;"),
      ("[a]b", "sub a' b by syn0;"),
      ("c[a]b", "sub c a' b by syn0;"),
      ("^c[a]b", "ignore sub @lc c a' b; sub c a' b by syn0;"),
      ("c[a]b$", "ignore sub c a' b @lc; sub c a' b by syn0;"),
      ("^c[a]b$", "ignore sub @lc c a' b; ignore sub c a' b @lc; sub c a' b by syn0;"),
      ("^c[az]b$", "ignore sub @lc c a' z b; ignore sub c a' z b @lc; sub c a' z' b by syn0;"),
    ] {
      let anterior = Anterior::decode(anterior_str).unwrap();
      let low_level = anterior.low_level(0);
      assert_eq!(
         itertools::Itertools::intersperse(low_level.iter().map(|s| crate::fea_parsing::render_fea_sub(s)), " ".to_owned()).collect::<String>(),
        should_str
      );
    }
  }
  
  #[test]
  fn test_anterior_correspondence_1() {
    for (anterior_str, word_str, should_str) in [
      ("[a]", "cat", "c{0}t"),
      ("a[a]", "aaa", "a{0}a"),
      ("a[a]", "aaaa", "a{0}a{0}"),
      ("^[a]", "cat", "cat"),
      ("[at]$", "cat", "c{0}"),
    ] {
      println!("{} {} {}", anterior_str, word_str, should_str);
      
      let word = aug_decode(word_str);
      let anterior = Anterior::decode(anterior_str).unwrap();
      
      let mut applied_directly = word.clone();
      anterior.apply(&mut applied_directly, 0);
      println!("directly: {}", aug_encode(&applied_directly));
      
      let mut applied_by_low_level = word.clone();
      let low_level = anterior.low_level(0);
      let sl = s2::SubstitutionList {
        lookups: vec![s2::Lookup {
          substitutions: low_level
        }]
      };
      s2::apply_all(&mut applied_by_low_level, &sl);
      println!("by low level: {}", aug_encode(&applied_by_low_level));
      
      assert_eq!(aug_encode(&applied_directly), should_str);
      assert_eq!(aug_encode(&applied_by_low_level), should_str);
      
      println!("");
    }
  }
  
  #[test]
  fn test_anterior_correspondence_2() {
    use rand::{distributions::{Uniform, Bernoulli}, prelude::Distribution};
    let mut rng = rand::thread_rng();
    
    let glyphs = crate::glyphs::decode("abʃ-'");
    
    let mut num_hit: usize = 0;
    
    for _ in 0 .. 10000 {
      let word: Vec<AugGlyph> = (0 .. Uniform::new_inclusive(1, 6).sample(&mut rng)).map(|_| {
        match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
          false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
          true => AugGlyph::Synthetic(0),
        }
      }).collect();
      
      let anterior = Anterior {
        at_start: Bernoulli::new(0.2).unwrap().sample(&mut rng),
        at_end: Bernoulli::new(0.2).unwrap().sample(&mut rng),
        not_at_start: false,
        not_at_end: false,
        pre_key: (0 .. Uniform::new_inclusive(0, 2).sample(&mut rng)).map(|_| {
          match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
            false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
            true => AugGlyph::Synthetic(0),
          }
        }).collect(),
        at_key: (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|_| {
          match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
            false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
            true => AugGlyph::Synthetic(0),
          }
        }).collect(),
        post_key: (0 .. Uniform::new_inclusive(0, 2).sample(&mut rng)).map(|_| {
          match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
            false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
            true => AugGlyph::Synthetic(0),
          }
        }).collect(),
      };
      
      let word_str = aug_encode(&word);
      let anterior_str = anterior.encode();
      
      let mut applied_directly = word.clone();
      if anterior.apply(&mut applied_directly, 1) {
        num_hit += 1;
      }
      
      let mut applied_by_low_level = word.clone();
      let low_level = anterior.low_level(1);
      let sl = s2::SubstitutionList {
        lookups: vec![s2::Lookup {
          substitutions: low_level
        }]
      };
      s2::apply_all(&mut applied_by_low_level, &sl);
      
      if aug_encode(&applied_directly) != aug_encode(&applied_by_low_level) {
        println!("{} {}", anterior_str, word_str);
        println!("directly: {}", aug_encode(&applied_directly));
        println!("by low level: {}", aug_encode(&applied_by_low_level));
        println!("");
      }
      assert_eq!(aug_encode(&applied_directly), aug_encode(&applied_by_low_level));
    }
    
    println!("num_hit = {}", num_hit);
  }
  
  #[test]
  fn test_anterior_correspondence_3() {
    let anterior_str = "[a]$";
    let word_str = "ab-";
    let should_str = "ab-";
    
    println!("{} {} {}", anterior_str, word_str, should_str);
    
    let word = aug_decode(word_str);
    let anterior = Anterior::decode(anterior_str).unwrap();
    
    let mut applied_directly = word.clone();
    anterior.apply(&mut applied_directly, 1);
    println!("directly: {}", aug_encode(&applied_directly));
    
    let mut applied_by_low_level = word.clone();
    let low_level = anterior.low_level(1);
    println!("low_level = {}", itertools::Itertools::intersperse(low_level.iter().map(|s| crate::fea_parsing::render_fea_sub(s)), " ".to_owned()).collect::<String>());
    let sl = s2::SubstitutionList {
      lookups: vec![s2::Lookup {
        substitutions: low_level
      }]
    };
    s2::apply_all(&mut applied_by_low_level, &sl);
    println!("by low level: {}", aug_encode(&applied_by_low_level));
    
    assert_eq!(aug_encode(&applied_directly), should_str);
    assert_eq!(aug_encode(&applied_by_low_level), should_str);
  }
  
  #[test]
  fn test_anterior_correspondence_4() {
    let anterior_str = "[aa]$";
    let word_str = "aaa";
    let should_str = "a{1}";
    
    println!("{} {} {}", anterior_str, word_str, should_str);
    
    let word = aug_decode(word_str);
    let anterior = Anterior::decode(anterior_str).unwrap();
    
    let mut applied_directly = word.clone();
    anterior.apply(&mut applied_directly, 1);
    println!("directly: {}", aug_encode(&applied_directly));
    
    let mut applied_by_low_level = word.clone();
    let low_level = anterior.low_level(1);
    println!("low_level = {}", itertools::Itertools::intersperse(low_level.iter().map(|s| crate::fea_parsing::render_fea_sub(s)), " ".to_owned()).collect::<String>());
    let sl = s2::SubstitutionList {
      lookups: vec![s2::Lookup {
        substitutions: low_level
      }]
    };
    s2::apply_all(&mut applied_by_low_level, &sl);
    println!("by low level: {}", aug_encode(&applied_by_low_level));
    
    assert_eq!(aug_encode(&applied_directly), should_str);
    assert_eq!(aug_encode(&applied_by_low_level), should_str);
  }
  
  #[test]
  fn test_posterior_correspondence_1() {
    use rand::{distributions::{Uniform, Bernoulli}, prelude::Distribution};
    let mut rng = rand::thread_rng();
    
    let glyphs = crate::glyphs::decode("abʃ-'");
    
    let mut num_hit: usize = 0;
    
    for _ in 0 .. 10000 {
      let word: Vec<AugGlyph> = (0 .. Uniform::new_inclusive(1, 6).sample(&mut rng)).map(|_| {
        match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
          false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
          true => AugGlyph::Synthetic(0),
        }
      }).collect();
      
      let posterior = Posterior {
        content: (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|_| {
          match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
            false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
            true => AugGlyph::Synthetic(0),
          }
        }).collect(),
      };
      
      let word_str = aug_encode(&word);
      let posterior_str = aug_encode(&posterior.content);
      
      println!("{} {}", posterior_str, word_str);
      
      let mut deapplied = word.clone();
      if posterior.deapply(&mut deapplied, 1) {
        num_hit += 1;
      }
      println!("deapplied: {}", aug_encode(&deapplied));
      
      let mut reapplied_directly = deapplied.clone();
      posterior.apply(&mut reapplied_directly, 1);
      
      let mut reapplied_by_low_level = deapplied.clone();
      let low_level = posterior.low_level(1);
      let sl = s2::SubstitutionList {
        lookups: vec![s2::Lookup {
          substitutions: vec![low_level]
        }]
      };
      s2::apply_all(&mut reapplied_by_low_level, &sl);
      
      println!("reapplied directly: {}", aug_encode(&reapplied_directly));
      println!("reapplied by low level: {}", aug_encode(&reapplied_by_low_level));
      assert_eq!(aug_encode(&reapplied_directly), aug_encode(&word));
      assert_eq!(aug_encode(&reapplied_by_low_level), aug_encode(&word));
      
      println!("");
    }
    
    println!("num_hit = {}", num_hit);
  }
  
  #[test]
  fn inner_apply_test_1() {
    let word = aug_decode("cat");
    let s = HLSubstitution::decode("c[a]→0→oa").unwrap();
    let mut result = word.clone();
    assert_eq!(s.apply(&mut result), true);
    assert_eq!(aug_encode(&result), "coat");
  }
  
  #[test]
  fn inner_apply_test_2() {
    let word = aug_decode("cat");
    let s = HLSubstitution::decode("c[a]→1→{0}a").unwrap();
    let mut result = word.clone();
    assert_eq!(s.apply(&mut result), true);
    assert_eq!(aug_encode(&result), "c{0}at");
  }
  
  #[test]
  fn inner_apply_test_3() {
    let word = aug_decode("cat");
    let s = HLSubstitution::decode("c[a]$→1→{0}a").unwrap();
    let mut result = word.clone();
    assert_eq!(s.apply(&mut result), false);
    assert_eq!(aug_encode(&result), "cat");
  }
  
  #[test]
  fn naive_separation_demo_1() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("[a]→1→d").unwrap(),
      ]
    };
    
    let word = aug_decode("ab");
    
    let mut working = word.clone();
    for s in &hl_slist.substitutions {
      s.apply_anterior(&mut working);
    }
    assert_eq!(aug_encode(&working), "{1}{0}");
    for s in hl_slist.substitutions.iter().rev() {
      s.apply_posterior(&mut working);
    }
    
    let directly = working;
    
    let mut anterior_lookup = vec![];
    let mut posterior_lookup = vec![];
    
    for s in &hl_slist.substitutions {
      anterior_lookup.extend(s.anterior_low_level());
    }
    
    for s in hl_slist.substitutions.iter().rev() {
      posterior_lookup.push(s.posterior_low_level());
    }
    
    let mut working = word.clone();
    s2::apply_all(&mut working, &s2::SubstitutionList {
      lookups: vec![
        s2::Lookup { substitutions: anterior_lookup },
        s2::Lookup { substitutions: posterior_lookup },
      ]
    });
    
    let by_low_level = working.clone();
    
    assert_eq!(aug_encode(&directly), "dc");
    assert_eq!(aug_encode(&by_low_level), "db");
  }
  
  #[test]
  fn complete_low_level_test_1() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("[a]→1→d").unwrap(),
      ]
    };
    
    let low_level = hl_slist.low_level();
    let low_level_rendered = crate::fea_parsing::render_fea_feature_body(&low_level);
    
    println!("{}", low_level_rendered);

    assert_eq!(low_level_rendered, "lookup l0 {
  sub a b' by syn0;
} l0;
lookup l1 {
  sub a' by syn1;
} l1;
lookup l2 {
  sub syn1' by d;
  sub syn0' by c;
} l2;
");
  }

  #[test]
  fn complete_low_level_test_2() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("a[c]→1→d").unwrap(),
      ]
    };
    
    let low_level = hl_slist.low_level();
    let low_level_rendered = crate::fea_parsing::render_fea_feature_body(&low_level);
    
    println!("{}", low_level_rendered);

    assert_eq!(low_level_rendered, "lookup l0 {
  sub a b' by syn0;
  sub a c' by syn1;
} l0;
lookup l1 {
  sub syn1' by d;
  sub syn0' by c;
} l1;
");
  }

  #[test]
  fn complete_low_level_correspondence_test_1() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("[a]→1→d").unwrap(),
      ]
    };
    
    let word = aug_decode("ab");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    s2::apply_all(&mut by_low_level, &hl_slist.low_level());
    
    assert_eq!(aug_encode(&directly), "dc");
    assert_eq!(aug_encode(&by_low_level), "dc");
  }
  
  #[test]
  fn complete_low_level_correspondence_test_2() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("a[c]→1→d").unwrap(),
      ]
    };
    
    let word = aug_decode("ab");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    s2::apply_all(&mut by_low_level, &hl_slist.low_level());
    
    assert_eq!(aug_encode(&directly), "ac");
    assert_eq!(aug_encode(&by_low_level), "ac");
  }

  #[test]
  fn complete_low_level_correspondence_test_3() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("a[b]→0→c").unwrap(),
        HLSubstitution::decode("a[{0}]→1→d").unwrap(),
      ]
    };
    
    let word = aug_decode("ab");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    s2::apply_all(&mut by_low_level, &hl_slist.low_level());
    
    assert_eq!(aug_encode(&directly), "ad");
    assert_eq!(aug_encode(&by_low_level), "ad");
  }

  #[test]
  fn complete_low_level_correspondence_test_3_1() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("[a]→0→a").unwrap(),
        HLSubstitution::decode("[b]a→1→a").unwrap(),
      ]
    };
    
    let word = aug_decode("ba");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    let low_level = hl_slist.low_level();
    println!("{}", crate::fea_parsing::render_fea_feature_body(&low_level));
    s2::apply_all(&mut by_low_level, &low_level);
    
    assert_eq!(aug_encode(&directly), aug_encode(&by_low_level));
  }

  #[test]
  fn complete_low_level_correspondence_test_3_2() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("[x]→0→p").unwrap(),
        HLSubstitution::decode("[a]→1→{0}").unwrap(),
        HLSubstitution::decode("{0}[b]→2→q").unwrap(),
      ]
    };
    
    let word = aug_decode("ab");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    let low_level = hl_slist.low_level();
    println!("{}", crate::fea_parsing::render_fea_feature_body(&low_level));
    s2::apply_all(&mut by_low_level, &low_level);
    
    assert_eq!(aug_encode(&directly), aug_encode(&by_low_level));
  }

  #[test]
  fn complete_low_level_correspondence_test_3_3() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("[-]→0→a").unwrap(),
        HLSubstitution::decode("[b]$→1→c").unwrap(),
      ]
    };
    
    let word = aug_decode("b-");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    let low_level = hl_slist.low_level();
    println!("{}", crate::fea_parsing::render_fea_feature_body(&low_level));
    s2::apply_all(&mut by_low_level, &low_level);
    
    assert_eq!(aug_encode(&directly), aug_encode(&by_low_level));
  }

  #[test]
  fn complete_low_level_correspondence_test_3_4() {
    let hl_slist = HLSubstitutionList {
      substitutions: vec![
        HLSubstitution::decode("^[a]→0→b").unwrap(),
        HLSubstitution::decode("[-]→1→c").unwrap(),
      ]
    };
    
    let word = aug_decode("-a");
    
    let mut directly = word.clone();
    hl_slist.apply(&mut directly);
    
    let mut by_low_level = word.clone();
    let low_level = hl_slist.low_level();
    println!("{}", crate::fea_parsing::render_fea_feature_body(&low_level));
    s2::apply_all(&mut by_low_level, &low_level);
    
    assert_eq!(aug_encode(&directly), aug_encode(&by_low_level));
  }

  #[test]
  fn complete_low_level_correspondence_test_4() {
    use rand::{distributions::{Uniform, Bernoulli}, prelude::Distribution};
    let mut rng = rand::thread_rng();
    
    let glyphs = crate::glyphs::decode("abc-");
    
    let mut num_hit: usize = 0;
    
    for _ in 0 .. 10000 {
      let hl_slist = HLSubstitutionList {
        substitutions: (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|i| {
          let anterior = Anterior {
            at_start: Bernoulli::new(0.2).unwrap().sample(&mut rng),
            at_end: Bernoulli::new(0.2).unwrap().sample(&mut rng),
            not_at_start: false,
            not_at_end: false,
            pre_key: (0 .. Uniform::new_inclusive(0, 2).sample(&mut rng)).map(|_| {
              if i == 0 {
                AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)])
              }
              else {
                match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
                  false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
                  true => AugGlyph::Synthetic(Uniform::new(0, i).sample(&mut rng)),
                }
              }
            }).collect(),
            at_key: (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|_| {
              if i == 0 {
                AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)])
              }
              else {
                match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
                  false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
                  true => AugGlyph::Synthetic(Uniform::new(0, i).sample(&mut rng)),
                }
              }
            }).collect(),
            post_key: (0 .. Uniform::new_inclusive(0, 2).sample(&mut rng)).map(|_| {
              if i == 0 {
                AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)])
              }
              else {
                match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
                  false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
                  true => AugGlyph::Synthetic(Uniform::new(0, i).sample(&mut rng)),
                }
              }
            }).collect(),
          };
          
          let posterior = Posterior {
            content: (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|_| {
              if i == 0 {
                AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)])
              }
              else {
                match Bernoulli::new(0.1).unwrap().sample(&mut rng) {
                  false => AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)]),
                  true => AugGlyph::Synthetic(Uniform::new(0, i).sample(&mut rng)),
                }
              }
            }).collect(),
          };
          
          HLSubstitution {
            anterior,
            posterior,
            mid: i
          }
        }).collect()
      };
      
      let word: Vec<AugGlyph> = (0 .. Uniform::new_inclusive(1, 3).sample(&mut rng)).map(|_| {
        AugGlyph::Real(glyphs[Uniform::new(0, glyphs.len()).sample(&mut rng)])
      }).collect();
      
      let mut directly = word.clone();
      if hl_slist.apply(&mut directly) { num_hit += 1 }
      
      let mut by_low_level = word.clone();
      let low_level = hl_slist.low_level();
      s2::apply_all(&mut by_low_level, &low_level);
      
      if aug_encode(&directly) != aug_encode(&by_low_level) {
        println!("word = {}", aug_encode(&word));
        println!("");
        
        println!("Subs:");
        for s in &hl_slist.substitutions {
          println!("  {}", s.encode());
        }
        println!("");
        
        println!("{}", crate::fea_parsing::render_fea_feature_body(&low_level));
        println!("");
      }
      
      assert_eq!(aug_encode(&directly), aug_encode(&by_low_level));
    }
    
    println!("num_hit = {}", num_hit);
  }
}

