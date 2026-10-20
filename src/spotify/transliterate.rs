use crate::ui::lyrics::LyricLine;

/// Romanizes synced lyric lines into Latin/English script without translating the semantic meaning.
/// (e.g. "मेरी" -> "meri", "तुम ही हो" -> "tum hi ho", "हम तेरे बिन" -> "hum tere bin",
/// "사랑해요" -> "saranghaeyo", "ありがとう" -> "arigatou", "Привет" -> "Privet").
///
/// Lines that are already in Latin/English letters (such as English songs, Spanish, or already-Romanized Hinglish)
/// are preserved exactly as-is.
pub fn romanize_lyrics(mut lines: Vec<LyricLine>) -> Vec<LyricLine> {
    for line in &mut lines {
        line.text = romanize_text(&line.text);
    }
    lines
}

/// Checks if all alphabetic characters in the lyric lines are already Latin script.
/// Used to prioritize community-curated Romanized/Hinglish/Romaji versions in LRCLIB search.
pub fn is_pure_latin(lines: &[LyricLine]) -> bool {
    let mut latin_count = 0;
    let mut non_latin_count = 0;

    for line in lines {
        for c in line.text.chars() {
            if c.is_alphabetic() {
                let u = c as u32;
                if u <= 0x024F {
                    latin_count += 1;
                } else {
                    non_latin_count += 1;
                }
            }
        }
    }

    latin_count > 0 && non_latin_count == 0
}

/// Romanizes a single line of text into English/Latin letters.
pub fn romanize_text(text: &str) -> String {
    // Fast path: if there are no non-Latin alphabetic characters, return text unchanged
    let has_non_latin = text.chars().any(|c| {
        c.is_alphabetic() && (c as u32 > 0x024F)
    });

    if !has_non_latin {
        return text.to_string();
    }

    // Split text into tokens while preserving whitespace and punctuation
    let mut result = String::with_capacity(text.len() * 2);
    let mut current_token = String::new();

    for c in text.chars() {
        if c.is_whitespace() {
            if !current_token.is_empty() {
                result.push_str(&romanize_token(&current_token));
                current_token.clear();
            }
            result.push(c);
        } else {
            current_token.push(c);
        }
    }

    if !current_token.is_empty() {
        result.push_str(&romanize_token(&current_token));
    }

    result
}

/// Romanize a single token (preserving attached punctuation like quotes, commas, parentheses).
fn romanize_token(token: &str) -> String {
    let (prefix, core, suffix) = split_affixes(token);
    if core.is_empty() {
        return token.to_string();
    }

    // 1. Check Hindi / Bollywood lyrics dictionary for exact natural Hinglish spelling
    if let Some(trans) = lookup_hindi_dictionary(core) {
        return format!("{}{}{}", prefix, trans, suffix);
    }

    // 2. Transliterate script by script
    let mut transliterated_core = String::new();
    let chars: Vec<char> = core.chars().collect();
    let mut i = 0;
    let n = chars.len();

    while i < n {
        let u = chars[i] as u32;

        if (0x0900..=0x097F).contains(&u) {
            // Devanagari sequence
            let start = i;
            while i < n && (0x0900..=0x097F).contains(&(chars[i] as u32)) {
                i += 1;
            }
            let dev_slice: String = chars[start..i].iter().collect();
            if let Some(dict_match) = lookup_hindi_dictionary(&dev_slice) {
                transliterated_core.push_str(dict_match);
            } else {
                transliterated_core.push_str(&devanagari_to_latin(&dev_slice));
            }
        } else if (0x0A00..=0x0A7F).contains(&u) {
            // Gurmukhi (Punjabi) sequence
            let start = i;
            while i < n && (0x0A00..=0x0A7F).contains(&(chars[i] as u32)) {
                i += 1;
            }
            let gur_slice: String = chars[start..i].iter().collect();
            transliterated_core.push_str(&gurmukhi_to_latin(&gur_slice));
        } else if (0x0980..=0x09FF).contains(&u) {
            // Bengali sequence
            let start = i;
            while i < n && (0x0980..=0x09FF).contains(&(chars[i] as u32)) {
                i += 1;
            }
            let ben_slice: String = chars[start..i].iter().collect();
            transliterated_core.push_str(&bengali_to_latin(&ben_slice));
        } else if (0x0400..=0x04FF).contains(&u) {
            // Cyrillic (Russian, Ukrainian, etc.)
            transliterated_core.push_str(cyrillic_char_to_latin(chars[i]));
            i += 1;
        } else if (0xAC00..=0xD7A3).contains(&u) {
            // Korean Hangul syllable
            transliterated_core.push_str(&hangul_syllable_to_latin(chars[i]));
            i += 1;
        } else if (0x3040..=0x30FF).contains(&u) {
            // Japanese Kana (Hiragana / Katakana)
            let start = i;
            while i < n && ((0x3040..=0x30FF).contains(&(chars[i] as u32)) || chars[i] == 'ー') {
                i += 1;
            }
            let kana_slice: String = chars[start..i].iter().collect();
            transliterated_core.push_str(&kana_to_romaji(&kana_slice));
        } else if (0x0600..=0x06FF).contains(&u) {
            // Arabic / Urdu
            transliterated_core.push_str(arabic_char_to_latin(chars[i]));
            i += 1;
        } else if (0x0370..=0x03FF).contains(&u) {
            // Greek
            transliterated_core.push_str(greek_char_to_latin(chars[i]));
            i += 1;
        } else {
            // Latin, ASCII, digits, punctuation, or other symbols
            transliterated_core.push(chars[i]);
            i += 1;
        }
    }

    format!("{}{}{}", prefix, transliterated_core, suffix)
}

/// Splits token into leading non-alphanumeric punctuation, core word, and trailing punctuation.
fn split_affixes(token: &str) -> (&str, &str, &str) {
    let mut first_char_idx = None;
    let mut last_char_idx = None;

    for (idx, c) in token.char_indices() {
        if c.is_alphabetic() || c.is_numeric() {
            if first_char_idx.is_none() {
                first_char_idx = Some(idx);
            }
            last_char_idx = Some(idx + c.len_utf8());
        }
    }

    match (first_char_idx, last_char_idx) {
        (Some(start), Some(end)) if start < end => {
            (&token[..start], &token[start..end], &token[end..])
        }
        _ => ("", token, ""),
    }
}

/// Comprehensive dictionary of frequent Hindi / Bollywood lyrics words for natural Hinglish spelling.
fn lookup_hindi_dictionary(word: &str) -> Option<&'static str> {
    match word {
        // Pronouns & Determiners
        "मेरी" => Some("meri"),
        "मेरा" => Some("mera"),
        "मेरे" => Some("mere"),
        "तेरे" => Some("tere"),
        "तेरी" => Some("teri"),
        "तेरा" => Some("tera"),
        "तुम" => Some("tum"),
        "तुमने" => Some("tumne"),
        "तुम्हें" => Some("tumhein"),
        "तुम्हारे" => Some("tumhare"),
        "तुम्हारी" => Some("tumhari"),
        "तुम्हारा" => Some("tumhara"),
        "तू" => Some("tu"),
        "तुझे" => Some("tujhe"),
        "तुझको" => Some("tujhko"),
        "तुझ" => Some("tujh"),
        "हम" => Some("hum"),
        "हमने" => Some("humne"),
        "हमे" => Some("humein"),
        "हमें" => Some("humein"),
        "हमारा" => Some("humara"),
        "हमारी" => Some("humari"),
        "हमारे" => Some("humare"),
        "मुझे" => Some("mujhe"),
        "मुझको" => Some("mujhko"),
        "मुझ" => Some("mujh"),
        "मैं" => Some("main"),
        "वो" => Some("woh"),
        "वह" => Some("woh"),
        "उसे" => Some("use"),
        "उसको" => Some("usko"),
        "उस" => Some("us"),
        "उन" => Some("un"),
        "उन्हें" => Some("unhein"),
        "उनका" => Some("unka"),
        "उनकी" => Some("unki"),
        "उनके" => Some("unke"),
        "ये" => Some("ye"),
        "यह" => Some("yeh"),
        "इस" => Some("is"),
        "इसे" => Some("ise"),
        "इसको" => Some("isko"),
        "इन" => Some("in"),
        "इन्हें" => Some("inhein"),
        "इनका" => Some("inka"),
        "इनकी" => Some("inki"),
        "इनके" => Some("inke"),
        "कौन" => Some("kaun"),
        "कोई" => Some("koi"),
        "कुछ" => Some("kuch"),
        "क्या" => Some("kya"),
        "क्यों" => Some("kyun"),
        "क्यूं" => Some("kyun"),
        "क्योंकि" => Some("kyunki"),
        "क्यूँकि" => Some("kyunki"),
        "कहाँ" => Some("kahan"),
        "यहाँ" => Some("yahan"),
        "वहाँ" => Some("wahan"),
        "जहाँ" => Some("jahan"),
        "अब" => Some("ab"),
        "तब" => Some("tab"),
        "कब" => Some("kab"),
        "जब" => Some("jab"),
        "सब" => Some("sab"),
        "कभी" => Some("kabhi"),
        "सभी" => Some("sabhi"),
        "अभी" => Some("abhi"),

        // Verbs & Particles
        "है" => Some("hai"),
        "हैं" => Some("hain"),
        "हो" => Some("ho"),
        "हू" => Some("hoon"),
        "हूँ" => Some("hoon"),
        "था" => Some("tha"),
        "थी" => Some("thi"),
        "थे" => Some("the"),
        "नहीं" => Some("nahi"),
        "नही" => Some("nahi"),
        "ना" => Some("na"),
        "रह" => Some("reh"),
        "रहा" => Some("raha"),
        "रही" => Some("rahi"),
        "रहे" => Some("rahe"),
        "रहना" => Some("rehna"),
        "रहती" => Some("rehti"),
        "रहता" => Some("rehta"),
        "रहते" => Some("rehte"),
        "सकते" => Some("sakte"),
        "सकती" => Some("sakti"),
        "सकता" => Some("sakta"),
        "सके" => Some("sake"),
        "कर" => Some("kar"),
        "करा" => Some("kara"),
        "करे" => Some("kare"),
        "करें" => Some("karein"),
        "करूं" => Some("karoon"),
        "करना" => Some("karna"),
        "किया" => Some("kiya"),
        "की" => Some("ki"),
        "के" => Some("ke"),
        "का" => Some("ka"),
        "को" => Some("ko"),
        "से" => Some("se"),
        "में" => Some("mein"),
        "पे" => Some("pe"),
        "पर" => Some("par"),
        "तक" => Some("tak"),
        "ने" => Some("ne"),
        "ले" => Some("le"),
        "लो" => Some("lo"),
        "लेना" => Some("lena"),
        "लिया" => Some("liya"),
        "दे" => Some("de"),
        "दो" => Some("do"),
        "देना" => Some("dena"),
        "दिया" => Some("diya"),
        "जा" => Some("jaa"),
        "जाना" => Some("jaana"),
        "जाने" => Some("jaane"),
        "जाऊं" => Some("jaaoon"),
        "जाएंगे" => Some("jaayenge"),
        "जायेंगे" => Some("jaayenge"),
        "जाए" => Some("jaaye"),
        "जाएं" => Some("jaayein"),
        "गया" => Some("gaya"),
        "गयी" => Some("gayi"),
        "गए" => Some("gaye"),
        "आ" => Some("aa"),
        "आना" => Some("aana"),
        "आने" => Some("aane"),
        "आया" => Some("aaya"),
        "आई" => Some("aayi"),
        "आए" => Some("aaye"),
        "आओ" => Some("aao"),
        "कह" => Some("keh"),
        "कहो" => Some("kaho"),
        "कहना" => Some("kehna"),
        "कहा" => Some("kaha"),
        "सुन" => Some("sun"),
        "सुनो" => Some("suno"),
        "सुना" => Some("suna"),
        "देख" => Some("dekh"),
        "देखो" => Some("dekho"),
        "देखा" => Some("dekha"),
        "मिल" => Some("mil"),
        "मिला" => Some("mila"),
        "मिले" => Some("mile"),
        "मिलना" => Some("milna"),
        "चल" => Some("chal"),
        "चलो" => Some("chalo"),
        "चला" => Some("chala"),
        "होके" => Some("hoke"),
        "होकर" => Some("hokar"),
        "होना" => Some("hona"),
        "होगा" => Some("hoga"),
        "होगी" => Some("hogi"),
        "होंगे" => Some("honge"),
        "जिएं" => Some("jiyein"),
        "जीएंगे" => Some("jiyenge"),
        "मरेंगे" => Some("marenge"),
        "मरना" => Some("marna"),
        "जीना" => Some("jeena"),

        // Lyrics Nouns, Adjectives & Emotions
        "दिल" => Some("dil"),
        "प्यार" => Some("pyaar"),
        "इश्क़" => Some("ishq"),
        "इश्क" => Some("ishq"),
        "मोहब्बत" => Some("mohabbat"),
        "आशिकी" => Some("aashiqui"),
        "आशिक" => Some("aashiq"),
        "दीवाना" => Some("deewana"),
        "दीवानी" => Some("deewani"),
        "सनम" => Some("sanam"),
        "जान" => Some("jaan"),
        "याद" => Some("yaad"),
        "यादों" => Some("yaadon"),
        "यादें" => Some("yaadein"),
        "बात" => Some("baat"),
        "बातों" => Some("baaton"),
        "बातें" => Some("baatein"),
        "रात" => Some("raat"),
        "रातों" => Some("raaton"),
        "रातें" => Some("raatein"),
        "दिन" => Some("din"),
        "पल" => Some("pal"),
        "हर" => Some("har"),
        "एक" => Some("ek"),
        "साथ" => Some("saath"),
        "पास" => Some("paas"),
        "दूर" => Some("door"),
        "ज़िंदगी" => Some("zindagi"),
        "जिंदगी" => Some("zindagi"),
        "ज़िन्दगी" => Some("zindagi"),
        "खुदा" => Some("khuda"),
        "दुआ" => Some("dua"),
        "वफा" => Some("wafa"),
        "बेवफा" => Some("bewafa"),
        "सजा" => Some("saza"),
        "मज़ा" => Some("maza"),
        "मजा" => Some("maza"),
        "नशा" => Some("nasha"),
        "हवा" => Some("hawa"),
        "हवाएं" => Some("hawayein"),
        "हवाएँ" => Some("hawayein"),
        "आंखें" => Some("aankhein"),
        "आँखें" => Some("aankhein"),
        "आंखों" => Some("aankhon"),
        "आँखों" => Some("aankhon"),
        "नज़र" => Some("nazar"),
        "नजर" => Some("nazar"),
        "चेहरा" => Some("chehra"),
        "दर्द" => Some("dard"),
        "आंसू" => Some("aansu"),
        "खुशी" => Some("khushi"),
        "हंसी" => Some("hansi"),
        "हसीं" => Some("haseen"),
        "सपना" => Some("sapna"),
        "सपने" => Some("sapne"),
        "दुनिया" => Some("duniya"),
        "जहान" => Some("jahan"),
        "यार" => Some("yaar"),
        "यारा" => Some("yaara"),
        "दोस्त" => Some("dost"),
        "चैन" => Some("chain"),
        "सुकून" => Some("sukoon"),
        "वजूद" => Some("wajood"),
        "जुदा" => Some("juda"),
        "जुदाई" => Some("judai"),
        "तन्हाई" => Some("tanhai"),
        "तन्हा" => Some("tanha"),
        "रास्ते" => Some("raaste"),
        "रास्ता" => Some("raasta"),
        "मंज़िल" => Some("manzil"),
        "मंजिल" => Some("manzil"),
        "सफ़र" => Some("safar"),
        "सफर" => Some("safar"),
        "मौसम" => Some("mausam"),
        "बारिश" => Some("baarish"),
        "गीत" => Some("geet"),
        "धड़कन" => Some("dhadkan"),
        "सांस" => Some("saans"),
        "सांसें" => Some("saansein"),
        "साँसें" => Some("saansein"),
        "रूठा" => Some("rootha"),
        "टूटा" => Some("toota"),
        "जैसे" => Some("jaise"),
        "वैसे" => Some("waise"),
        "ऐसे" => Some("aise"),
        "कैसे" => Some("kaise"),
        "सिर्फ" => Some("sirf"),
        "बस" => Some("bas"),
        "बहुत" => Some("bahut"),
        "ज्यादा" => Some("zyada"),
        "कम" => Some("kam"),
        "और" => Some("aur"),
        "या" => Some("ya"),
        "भी" => Some("bhi"),
        "तो" => Some("toh"),
        "वही" => Some("wahi"),
        "यही" => Some("yahi"),
        "अपना" => Some("apna"),
        "अपनी" => Some("apni"),
        "अपने" => Some("apne"),
        "बिना" => Some("bina"),
        "बिन" => Some("bin"),
        "आगे" => Some("aage"),
        "पीछे" => Some("peeche"),
        "ऊपर" => Some("upar"),
        "नीचे" => Some("neeche"),
        "पहला" => Some("pehla"),
        "पहली" => Some("pehli"),
        "आखिरी" => Some("aakhri"),
        "आख़िरी" => Some("aakhri"),
        "अगर" => Some("agar"),
        "मगर" => Some("magar"),
        "लेकिन" => Some("lekin"),
        "फिर" => Some("phir"),
        "दियां" => Some("diyan"),
        "गल्लां" => Some("gallan"),
        _ => None,
    }
}

/// Algorithmic Devanagari transliteration with Hindi schwa syncope rules.
fn devanagari_to_latin(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;

    while i < n {
        let mut code = chars[i] as u32;

        // Check if followed by nuqta (0x093C)
        if i + 1 < n && chars[i + 1] as u32 == 0x093C {
            code = match code {
                0x0915 => 0x0958, // क -> क़ (q)
                0x0916 => 0x0959, // ख -> ख़ (kh)
                0x0917 => 0x095A, // ग -> ग़ (gh)
                0x091C => 0x095B, // ज -> ज़ (z)
                0x0921 => 0x095C, // ड -> ड़ (d)
                0x0922 => 0x095D, // ढ -> ढ़ (rh)
                0x092B => 0x095E, // फ -> फ़ (f)
                _ => code,
            };
            i += 1;
        }

        // 1. Independent Vowels
        if let Some(v) = devanagari_independent_vowel(code) {
            out.push_str(v);
        }
        // 2. Matras (dependent vowels)
        else if let Some(m) = devanagari_matra(code) {
            out.push_str(m);
        }
        // 3. Consonants
        else if let Some(c_str) = devanagari_consonant(code) {
            let has_next = i + 1 < n;
            let next_code = if has_next { chars[i + 1] as u32 } else { 0 };

            if has_next && next_code == 0x094D {
                // Virama (halant) suppresses inherent 'a'
                out.push_str(c_str);
                i += 1; // skip virama
            } else if has_next && is_devanagari_matra(next_code) {
                // Followed by matra: no inherent 'a'
                out.push_str(c_str);
            } else if !has_next || !is_devanagari_letter(next_code) {
                // End of word or boundary: schwa deletion in modern Hindi
                out.push_str(c_str);
            } else {
                // Medial consonant: check schwa syncope (e.g. करना -> karna)
                if i + 2 < n
                    && is_devanagari_consonant(next_code)
                    && (is_devanagari_matra(chars[i + 2] as u32) || chars[i + 2] as u32 == 0x094D)
                {
                    if i > 0 && is_devanagari_letter(chars[i - 1] as u32) {
                        out.push_str(c_str);
                    } else {
                        out.push_str(c_str);
                        out.push('a');
                    }
                } else {
                    out.push_str(c_str);
                    out.push('a');
                }
            }
        }
        // 4. Anusvara & Chandrabindu
        else if code == 0x0901 || code == 0x0902 {
            out.push('n');
        }
        // 5. Visarga
        else if code == 0x0903 {
            out.push('h');
        }
        // 6. Avagraha (ऽ)
        else if code == 0x093D {
            // elongated sound, ignore or repeat
        } else {
            out.push(chars[i]);
        }

        i += 1;
    }

    out
}

fn devanagari_independent_vowel(code: u32) -> Option<&'static str> {
    match code {
        0x0905 => Some("a"),
        0x0906 => Some("aa"),
        0x0907 => Some("i"),
        0x0908 => Some("ee"),
        0x0909 => Some("u"),
        0x090A => Some("oo"),
        0x090B => Some("ri"),
        0x090E => Some("e"),
        0x090F => Some("e"),
        0x0910 => Some("ai"),
        0x0912 => Some("o"),
        0x0913 => Some("o"),
        0x0914 => Some("au"),
        _ => None,
    }
}

fn devanagari_matra(code: u32) -> Option<&'static str> {
    match code {
        0x093E => Some("a"),  // ा
        0x093F => Some("i"),  // ि
        0x0940 => Some("i"),  // ी
        0x0941 => Some("u"),  // ु
        0x0942 => Some("u"),  // ू
        0x0943 => Some("ri"), // ृ
        0x0947 => Some("e"),  // े
        0x0948 => Some("ai"), // ै
        0x094B => Some("o"),  // ो
        0x094C => Some("au"), // ौ
        _ => None,
    }
}

fn is_devanagari_matra(code: u32) -> bool {
    (0x093E..=0x094C).contains(&code)
}

fn is_devanagari_consonant(code: u32) -> bool {
    devanagari_consonant(code).is_some()
}

fn is_devanagari_letter(code: u32) -> bool {
    (0x0905..=0x0939).contains(&code) || (0x0958..=0x095F).contains(&code)
}

fn devanagari_consonant(code: u32) -> Option<&'static str> {
    match code {
        0x0915 => Some("k"),
        0x0916 => Some("kh"),
        0x0917 => Some("g"),
        0x0918 => Some("gh"),
        0x0919 => Some("ng"),
        0x091A => Some("ch"),
        0x091B => Some("chh"),
        0x091C => Some("j"),
        0x091D => Some("jh"),
        0x091E => Some("ny"),
        0x091F => Some("t"),
        0x0920 => Some("th"),
        0x0921 => Some("d"),
        0x0922 => Some("dh"),
        0x0923 => Some("n"),
        0x0924 => Some("t"),
        0x0925 => Some("th"),
        0x0926 => Some("d"),
        0x0927 => Some("dh"),
        0x0928 => Some("n"),
        0x0929 => Some("n"),
        0x092A => Some("p"),
        0x092B => Some("ph"),
        0x092C => Some("b"),
        0x092D => Some("bh"),
        0x092E => Some("m"),
        0x092F => Some("y"),
        0x0930 => Some("r"),
        0x0931 => Some("r"),
        0x0932 => Some("l"),
        0x0933 => Some("l"),
        0x0935 => Some("v"),
        0x0936 => Some("sh"),
        0x0937 => Some("sh"),
        0x0938 => Some("s"),
        0x0939 => Some("h"),
        0x0958 => Some("q"),
        0x0959 => Some("kh"),
        0x095A => Some("gh"),
        0x095B => Some("z"),
        0x095C => Some("d"),
        0x095D => Some("rh"),
        0x095E => Some("f"),
        0x095F => Some("y"),
        _ => None,
    }
}

/// Algorithmic Gurmukhi (Punjabi) transliteration to Latin.
fn gurmukhi_to_latin(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;

    while i < n {
        let code = chars[i] as u32;

        match code {
            // Vowels
            0x0A05 => out.push_str("a"),
            0x0A06 => out.push_str("aa"),
            0x0A07 => out.push_str("i"),
            0x0A08 => out.push_str("ee"),
            0x0A09 => out.push_str("u"),
            0x0A0A => out.push_str("oo"),
            0x0A0F => out.push_str("e"),
            0x0A10 => out.push_str("ai"),
            0x0A13 => out.push_str("o"),
            0x0A14 => out.push_str("au"),

            // Matras
            0x0A3E => out.push_str("a"),
            0x0A3F => out.push_str("i"),
            0x0A40 => out.push_str("i"),
            0x0A41 => out.push_str("u"),
            0x0A42 => out.push_str("u"),
            0x0A47 => out.push_str("e"),
            0x0A48 => out.push_str("ai"),
            0x0A4B => out.push_str("o"),
            0x0A4C => out.push_str("au"),

            // Consonants
            0x0A15 => out.push_str("k"),
            0x0A16 => out.push_str("kh"),
            0x0A17 => out.push_str("g"),
            0x0A18 => out.push_str("gh"),
            0x0A19 => out.push_str("ng"),
            0x0A1A => out.push_str("ch"),
            0x0A1B => out.push_str("chh"),
            0x0A1C => out.push_str("j"),
            0x0A1D => out.push_str("jh"),
            0x0A1E => out.push_str("ny"),
            0x0A1F => out.push_str("t"),
            0x0A20 => out.push_str("th"),
            0x0A21 => out.push_str("d"),
            0x0A22 => out.push_str("dh"),
            0x0A23 => out.push_str("n"),
            0x0A24 => out.push_str("t"),
            0x0A25 => out.push_str("th"),
            0x0A26 => out.push_str("d"),
            0x0A27 => out.push_str("dh"),
            0x0A28 => out.push_str("n"),
            0x0A2A => out.push_str("p"),
            0x0A2B => out.push_str("ph"),
            0x0A2C => out.push_str("b"),
            0x0A2D => out.push_str("bh"),
            0x0A2E => out.push_str("m"),
            0x0A2F => out.push_str("y"),
            0x0A30 => out.push_str("r"),
            0x0A32 => out.push_str("l"),
            0x0A35 => out.push_str("v"),
            0x0A38 => out.push_str("s"),
            0x0A39 => out.push_str("h"),
            0x0A59 => out.push_str("kh"),
            0x0A5A => out.push_str("gh"),
            0x0A5B => out.push_str("z"),
            0x0A5C => out.push_str("r"),
            0x0A5E => out.push_str("f"),

            // Nasals & Addak
            0x0A70 | 0x0A02 => out.push_str("n"),
            0x0A71 => {
                // Addak doubles following consonant
                if i + 1 < n {
                    let next_c = chars[i + 1] as u32;
                    let next_str = match next_c {
                        0x0A15 => "k", 0x0A17 => "g", 0x0A1A => "ch", 0x0A1C => "j",
                        0x0A1F => "t", 0x0A21 => "d", 0x0A24 => "t", 0x0A26 => "d",
                        0x0A2A => "p", 0x0A2C => "b", 0x0A2E => "m", 0x0A30 => "r",
                        0x0A32 => "l", 0x0A38 => "s", _ => "",
                    };
                    out.push_str(next_str);
                }
            }
            0x0A4D => {} // virama
            _ => out.push(chars[i]),
        }

        i += 1;
    }

    out
}

/// Algorithmic Bengali transliteration to Latin.
fn bengali_to_latin(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;

    while i < n {
        let code = chars[i] as u32;

        match code {
            0x0985 => out.push_str("o"),
            0x0986 => out.push_str("aa"),
            0x0987 => out.push_str("i"),
            0x0988 => out.push_str("ee"),
            0x0989 => out.push_str("u"),
            0x098A => out.push_str("oo"),
            0x098F => out.push_str("e"),
            0x0990 => out.push_str("oi"),
            0x0993 => out.push_str("o"),
            0x0994 => out.push_str("ou"),

            0x09BE => out.push_str("a"),
            0x09BF => out.push_str("i"),
            0x09C0 => out.push_str("i"),
            0x09C1 => out.push_str("u"),
            0x09C2 => out.push_str("u"),
            0x09C7 => out.push_str("e"),
            0x09C8 => out.push_str("oi"),
            0x09CB => out.push_str("o"),
            0x09CC => out.push_str("ou"),

            0x0995 => out.push_str("k"),
            0x0996 => out.push_str("kh"),
            0x0997 => out.push_str("g"),
            0x0998 => out.push_str("gh"),
            0x099A => out.push_str("ch"),
            0x099B => out.push_str("chh"),
            0x099C => out.push_str("j"),
            0x099D => out.push_str("jh"),
            0x099F => out.push_str("t"),
            0x09A0 => out.push_str("th"),
            0x09A1 => out.push_str("d"),
            0x09A2 => out.push_str("dh"),
            0x09A3 => out.push_str("n"),
            0x09A4 => out.push_str("t"),
            0x09A5 => out.push_str("th"),
            0x09A6 => out.push_str("d"),
            0x09A7 => out.push_str("dh"),
            0x09A8 => out.push_str("n"),
            0x09AA => out.push_str("p"),
            0x09AB => out.push_str("ph"),
            0x09AC => out.push_str("b"),
            0x09AD => out.push_str("bh"),
            0x09AE => out.push_str("m"),
            0x09AF => out.push_str("j"),
            0x09B0 => out.push_str("r"),
            0x09B2 => out.push_str("l"),
            0x09B6 => out.push_str("sh"),
            0x09B7 => out.push_str("sh"),
            0x09B8 => out.push_str("s"),
            0x09B9 => out.push_str("h"),
            0x09DC => out.push_str("r"),
            0x09DD => out.push_str("rh"),
            0x09DF => out.push_str("y"),
            0x0981 | 0x0982 => out.push_str("n"),
            0x0983 => out.push_str("h"),
            0x09CD => {} // virama
            _ => out.push(chars[i]),
        }

        i += 1;
    }

    out
}

/// Cyrillic character transliteration.
fn cyrillic_char_to_latin(c: char) -> &'static str {
    match c {
        'А' => "A", 'а' => "a",
        'Б' => "B", 'б' => "b",
        'В' => "V", 'в' => "v",
        'Г' => "G", 'г' => "g",
        'Д' => "D", 'д' => "d",
        'Е' => "E", 'е' => "e",
        'Ё' => "Yo", 'ё' => "yo",
        'Ж' => "Zh", 'ж' => "zh",
        'З' => "Z", 'з' => "z",
        'И' => "I", 'и' => "i",
        'Й' => "Y", 'й' => "y",
        'К' => "K", 'к' => "k",
        'Л' => "L", 'л' => "l",
        'М' => "M", 'м' => "m",
        'Н' => "N", 'н' => "n",
        'О' => "O", 'о' => "o",
        'П' => "P", 'п' => "p",
        'Р' => "R", 'р' => "r",
        'С' => "S", 'с' => "s",
        'Т' => "T", 'т' => "t",
        'У' => "U", 'у' => "u",
        'Ф' => "F", 'ф' => "f",
        'Х' => "Kh", 'х' => "kh",
        'Ц' => "Ts", 'ц' => "ts",
        'Ч' => "Ch", 'ч' => "ch",
        'Ш' => "Sh", 'ш' => "sh",
        'Щ' => "Shch", 'щ' => "shch",
        'Ъ' | 'ъ' => "",
        'Ы' => "Y", 'ы' => "y",
        'Ь' | 'ь' => "",
        'Э' => "E", 'э' => "e",
        'Ю' => "Yu", 'ю' => "yu",
        'Я' => "Ya", 'я' => "ya",
        // Ukrainian / Belarusian
        'Є' => "Ye", 'є' => "ye",
        'І' => "I", 'і' => "i",
        'Ї' => "Yi", 'ї' => "yi",
        'Ґ' => "G", 'ґ' => "g",
        'Ў' => "U", 'ў' => "u",
        _ => "",
    }
}

/// Korean Hangul Syllable mathematical decomposition (Revised Romanization of Korean).
fn hangul_syllable_to_latin(c: char) -> String {
    let code = c as u32;
    if !(0xAC00..=0xD7A3).contains(&code) {
        return c.to_string();
    }

    const CHOSEONG: [&str; 19] = [
        "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t",
        "p", "h",
    ];
    const JUNGSEONG: [&str; 21] = [
        "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo",
        "we", "wi", "yu", "eu", "ui", "i",
    ];
    const JONGSEONG: [&str; 28] = [
        "", "k", "k", "ks", "n", "nj", "nh", "t", "l", "lg", "lm", "lb", "ls", "lt", "lp", "lh",
        "m", "p", "bs", "t", "t", "ng", "t", "t", "k", "t", "p", "h",
    ];

    let syl_idx = code - 0xAC00;
    let cho = (syl_idx / (21 * 28)) as usize;
    let jung = ((syl_idx % (21 * 28)) / 28) as usize;
    let jong = (syl_idx % 28) as usize;

    format!("{}{}{}", CHOSEONG[cho], JUNGSEONG[jung], JONGSEONG[jong])
}

/// Japanese Kana (Hiragana and Katakana) to Romaji.
fn kana_to_romaji(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;

    while i < n {
        // Check 2-character combo (youon: kya, sha, etc.)
        if i + 1 < n {
            let combo: String = chars[i..=i + 1].iter().collect();
            if let Some(rom) = lookup_kana_combo(&combo) {
                out.push_str(rom);
                i += 2;
                continue;
            }
        }

        let c = chars[i];
        if c == 'っ' || c == 'ッ' {
            // Sokuon doubles the consonant of next syllable
            if i + 1 < n {
                let next_slice = if i + 2 < n {
                    let two: String = chars[i + 1..=i + 2].iter().collect();
                    lookup_kana_combo(&two).or_else(|| lookup_kana_single(chars[i + 1]))
                } else {
                    lookup_kana_single(chars[i + 1])
                };
                if let Some(r) = next_slice {
                    if let Some(first_ch) = r.chars().next() {
                        out.push(first_ch);
                    }
                }
            }
            i += 1;
            continue;
        }

        if c == 'ー' {
            // Chouonpu prolongs preceding vowel
            if let Some(last_ch) = out.chars().last() {
                if matches!(last_ch, 'a' | 'e' | 'i' | 'o' | 'u') {
                    out.push(last_ch);
                }
            }
            i += 1;
            continue;
        }

        if let Some(rom) = lookup_kana_single(c) {
            out.push_str(rom);
        } else {
            out.push(c);
        }

        i += 1;
    }

    out
}

fn lookup_kana_single(c: char) -> Option<&'static str> {
    match c {
        // Hiragana
        'あ' => Some("a"), 'い' => Some("i"), 'う' => Some("u"), 'え' => Some("e"), 'お' => Some("o"),
        'か' => Some("ka"), 'き' => Some("ki"), 'く' => Some("ku"), 'け' => Some("ke"), 'こ' => Some("ko"),
        'さ' => Some("sa"), 'し' => Some("shi"), 'す' => Some("su"), 'せ' => Some("se"), 'そ' => Some("so"),
        'た' => Some("ta"), 'ち' => Some("chi"), 'つ' => Some("tsu"), 'て' => Some("te"), 'と' => Some("to"),
        'な' => Some("na"), 'に' => Some("ni"), 'ぬ' => Some("nu"), 'ね' => Some("ne"), 'の' => Some("no"),
        'は' => Some("ha"), 'ひ' => Some("hi"), 'ふ' => Some("fu"), 'へ' => Some("he"), 'ほ' => Some("ho"),
        'ま' => Some("ma"), 'み' => Some("mi"), 'む' => Some("mu"), 'め' => Some("me"), 'も' => Some("mo"),
        'や' => Some("ya"), 'ゆ' => Some("yu"), 'よ' => Some("yo"),
        'ら' => Some("ra"), 'り' => Some("ri"), 'る' => Some("ru"), 'れ' => Some("re"), 'ろ' => Some("ro"),
        'わ' => Some("wa"), 'を' => Some("wo"), 'ん' => Some("n"),
        'が' => Some("ga"), 'ぎ' => Some("gi"), 'ぐ' => Some("gu"), 'げ' => Some("ge"), 'ご' => Some("go"),
        'ざ' => Some("za"), 'じ' => Some("ji"), 'ず' => Some("zu"), 'ぜ' => Some("ze"), 'ぞ' => Some("zo"),
        'だ' => Some("da"), 'ぢ' => Some("ji"), 'づ' => Some("zu"), 'で' => Some("de"), 'ど' => Some("do"),
        'ば' => Some("ba"), 'び' => Some("bi"), 'ぶ' => Some("bu"), 'べ' => Some("be"), 'ぼ' => Some("bo"),
        'ぱ' => Some("pa"), 'ぴ' => Some("pi"), 'ぷ' => Some("pu"), 'ぺ' => Some("pe"), 'ぽ' => Some("po"),

        // Katakana
        'ア' => Some("a"), 'イ' => Some("i"), 'ウ' => Some("u"), 'エ' => Some("e"), 'オ' => Some("o"),
        'カ' => Some("ka"), 'キ' => Some("ki"), 'ク' => Some("ku"), 'ケ' => Some("ke"), 'コ' => Some("ko"),
        'サ' => Some("sa"), 'シ' => Some("shi"), 'ス' => Some("su"), 'セ' => Some("se"), 'ソ' => Some("so"),
        'タ' => Some("ta"), 'チ' => Some("chi"), 'ツ' => Some("tsu"), 'テ' => Some("te"), 'ト' => Some("to"),
        'ナ' => Some("na"), 'ニ' => Some("ni"), 'ヌ' => Some("nu"), 'ネ' => Some("ne"), 'ノ' => Some("no"),
        'ハ' => Some("ha"), 'ヒ' => Some("hi"), 'フ' => Some("fu"), 'ヘ' => Some("he"), 'ホ' => Some("ho"),
        'マ' => Some("ma"), 'ミ' => Some("mi"), 'ム' => Some("mu"), 'メ' => Some("me"), 'モ' => Some("mo"),
        'ヤ' => Some("ya"), 'ユ' => Some("yu"), 'ヨ' => Some("yo"),
        'ラ' => Some("ra"), 'リ' => Some("ri"), 'ル' => Some("ru"), 'レ' => Some("re"), 'ロ' => Some("ro"),
        'ワ' => Some("wa"), 'ヲ' => Some("wo"), 'ン' => Some("n"),
        'ガ' => Some("ga"), 'ギ' => Some("gi"), 'グ' => Some("gu"), 'ゲ' => Some("ge"), 'ゴ' => Some("go"),
        'ザ' => Some("za"), 'ジ' => Some("ji"), 'ズ' => Some("zu"), 'ゼ' => Some("ze"), 'ゾ' => Some("zo"),
        'ダ' => Some("da"), 'ヂ' => Some("ji"), 'ヅ' => Some("zu"), 'デ' => Some("de"), 'ド' => Some("do"),
        'バ' => Some("ba"), 'ビ' => Some("bi"), 'ブ' => Some("bu"), 'ベ' => Some("be"), 'ボ' => Some("bo"),
        'パ' => Some("pa"), 'ピ' => Some("pi"), 'プ' => Some("pu"), 'ペ' => Some("pe"), 'ポ' => Some("po"),

        _ => None,
    }
}

fn lookup_kana_combo(combo: &str) -> Option<&'static str> {
    match combo {
        // Hiragana combos
        "きゃ" => Some("kya"), "きゅ" => Some("kyu"), "きょ" => Some("kyo"),
        "しゃ" => Some("sha"), "しゅ" => Some("shu"), "しょ" => Some("sho"),
        "ちゃ" => Some("cha"), "ちゅ" => Some("chu"), "ちょ" => Some("cho"),
        "にゃ" => Some("nya"), "にゅ" => Some("nyu"), "にょ" => Some("nyo"),
        "ひゃ" => Some("hya"), "ひゅ" => Some("hyu"), "ひょ" => Some("hyo"),
        "みゃ" => Some("mya"), "みゅ" => Some("myu"), "みょ" => Some("myo"),
        "りゃ" => Some("rya"), "りゅ" => Some("ryu"), "りょ" => Some("ryo"),
        "ぎゃ" => Some("gya"), "ぎゅ" => Some("gyu"), "ぎょ" => Some("gyo"),
        "じゃ" => Some("ja"),  "じゅ" => Some("ju"),  "じょ" => Some("jo"),
        "びゃ" => Some("bya"), "びゅ" => Some("byu"), "びょ" => Some("byo"),
        "ぴゃ" => Some("pya"), "ぴゅ" => Some("pyu"), "ぴょ" => Some("pyo"),

        // Katakana combos
        "キャ" => Some("kya"), "キュ" => Some("kyu"), "キョ" => Some("kyo"),
        "シャ" => Some("sha"), "シュ" => Some("shu"), "ショ" => Some("sho"),
        "チャ" => Some("cha"), "チュ" => Some("chu"), "チョ" => Some("cho"),
        "ニャ" => Some("nya"), "ニュ" => Some("nyu"), "ニョ" => Some("nyo"),
        "ヒャ" => Some("hya"), "ヒュ" => Some("hyu"), "ヒョ" => Some("hyo"),
        "ミャ" => Some("mya"), "ミュ" => Some("myu"), "ミョ" => Some("myo"),
        "リャ" => Some("rya"), "リュ" => Some("ryu"), "リョ" => Some("ryo"),
        "ギャ" => Some("gya"), "ギュ" => Some("gyu"), "ギョ" => Some("gyo"),
        "ジャ" => Some("ja"),  "ジュ" => Some("ju"),  "ジョ" => Some("jo"),
        "ビャ" => Some("bya"), "ビュ" => Some("byu"), "ビョ" => Some("byo"),
        "ピャ" => Some("pya"), "ピュ" => Some("pyu"), "ピョ" => Some("pyo"),
        _ => None,
    }
}

/// Arabic / Urdu character transliteration to Latin.
fn arabic_char_to_latin(c: char) -> &'static str {
    match c {
        'ا' | 'آ' => "a",
        'ب' => "b",
        'پ' => "p",
        'ت' | 'ٹ' => "t",
        'ث' => "s",
        'ج' => "j",
        'چ' => "ch",
        'ح' => "h",
        'خ' => "kh",
        'د' | 'ڈ' => "d",
        'ذ' => "z",
        'ر' | 'ڑ' => "r",
        'ز' | 'ژ' => "z",
        'س' => "s",
        'ش' => "sh",
        'ص' => "s",
        'ض' => "z",
        'ط' => "t",
        'ظ' => "z",
        'ع' => "a",
        'غ' => "gh",
        'ف' => "f",
        'ق' => "q",
        'ک' | 'ك' => "k",
        'گ' => "g",
        'ل' => "l",
        'م' => "m",
        'ن' | 'ں' => "n",
        'و' => "o",
        'ہ' | 'ھ' => "h",
        'ی' | 'ي' => "i",
        'ے' => "e",
        'ء' => "'",
        _ => "",
    }
}

/// Greek character transliteration to Latin.
fn greek_char_to_latin(c: char) -> &'static str {
    match c {
        'Α' | 'α' => "a",
        'Β' | 'β' => "v",
        'Γ' | 'γ' => "g",
        'Δ' | 'δ' => "d",
        'Ε' | 'ε' | 'Έ' | 'έ' => "e",
        'Ζ' | 'ζ' => "z",
        'Η' | 'η' | 'Ή' | 'ή' => "i",
        'Θ' | 'θ' => "th",
        'Ι' | 'ι' | 'Ί' | 'ί' => "i",
        'Κ' | 'κ' => "k",
        'Λ' | 'λ' => "l",
        'Μ' | 'μ' => "m",
        'Ν' | 'ν' => "n",
        'Ξ' | 'ξ' => "x",
        'Ο' | 'ο' | 'Ό' | 'ό' => "o",
        'Π' | 'π' => "p",
        'Ρ' | 'ρ' => "r",
        'Σ' | 'σ' | 'ς' => "s",
        'Τ' | 'τ' => "t",
        'Υ' | 'υ' | 'Ύ' | 'ύ' => "y",
        'Φ' | 'φ' => "f",
        'Χ' | 'χ' => "ch",
        'Ψ' | 'ψ' => "ps",
        'Ω' | 'ω' | 'Ώ' | 'ώ' => "o",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hindi_romanization() {
        assert_eq!(romanize_text("मेरी"), "meri");
        assert_eq!(romanize_text("मेरा प्यार"), "mera pyaar");
        assert_eq!(
            romanize_text("हम तेरे बिन अब रह नहीं सकते"),
            "hum tere bin ab reh nahi sakte"
        );
        assert_eq!(
            romanize_text("तेरे बिना क्या वजूद मेरा"),
            "tere bina kya wajood mera"
        );
        assert_eq!(
            romanize_text("तुझ से जुदा अगर हो जायेंगे"),
            "tujh se juda agar ho jaayenge"
        );
        assert_eq!(
            romanize_text("क्योंकि तुम ही हो, अब तुम ही हो!"),
            "kyunki tum hi ho, ab tum hi ho!"
        );
        assert_eq!(
            romanize_text("जिंदगी अब तुम ही हो"),
            "zindagi ab tum hi ho"
        );
        assert_eq!(
            romanize_text("(मेरी)"),
            "(meri)"
        );
    }

    #[test]
    fn test_latin_unchanged() {
        let eng = "Hum tere bin ab reh nahi sakte";
        assert_eq!(romanize_text(eng), eng);
        let song = "Hello from the other side, I must have called a thousand times";
        assert_eq!(romanize_text(song), song);
        let spanish = "Despacito quiero respirar tu cuello despacito";
        assert_eq!(romanize_text(spanish), spanish);
    }

    #[test]
    fn test_korean_hangul() {
        assert_eq!(romanize_text("사랑해요"), "saranghaeyo");
        assert_eq!(romanize_text("안녕하세요"), "annyeonghaseyo");
    }

    #[test]
    fn test_japanese_kana() {
        assert_eq!(romanize_text("ありがとう"), "arigatou");
        assert_eq!(romanize_text("サヨナラ"), "sayonara");
        assert_eq!(romanize_text("いっせーのーせで"), "isseenoosede");
    }

    #[test]
    fn test_cyrillic() {
        assert_eq!(romanize_text("Привет мир"), "Privet mir");
        assert_eq!(romanize_text("Я люблю тебя"), "Ya lyublyu tebya");
    }

    #[test]
    fn test_punjabi_gurmukhi() {
        assert_eq!(romanize_text("ਮੇਰੀ"), "meri");
        assert_eq!(romanize_text("ਤੇਰੇ"), "tere");
    }
}
