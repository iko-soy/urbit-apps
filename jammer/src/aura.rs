//! Hoon's text renderings of atoms (`scow`), as used by marks that print
//! nouns into files: `@ud`, `@ux`, `@uv` and `@p`.

use crate::mug::murmur3_32;
use crate::noun::trim;

/// `(scow %ud n)`: decimal, with a `.` every three digits (`1.024`).
pub fn ud(atom: &[u8]) -> String {
    // Repeatedly divide a base-2^32 number by 10^9.
    let atom = trim(atom);
    let mut limbs: Vec<u32> = atom
        .chunks(4)
        .map(|c| {
            let mut buf = [0u8; 4];
            buf[..c.len()].copy_from_slice(c);
            u32::from_le_bytes(buf)
        })
        .collect();
    let mut chunks = Vec::new();
    while limbs.iter().any(|&l| l != 0) {
        let mut rem = 0u64;
        for limb in limbs.iter_mut().rev() {
            let cur = (rem << 32) | u64::from(*limb);
            *limb = (cur / 1_000_000_000) as u32;
            rem = cur % 1_000_000_000;
        }
        chunks.push(rem as u32);
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
    }
    let mut digits = match chunks.pop() {
        None => return "0".into(),
        Some(top) => top.to_string(),
    };
    for chunk in chunks.iter().rev() {
        digits.push_str(&format!("{chunk:09}"));
    }
    group(&digits, 3)
}

/// `(scow %ux n)`: `0x` and hexadecimal, with a `.` every four digits.
pub fn ux(atom: &[u8]) -> String {
    let atom = trim(atom);
    let mut digits: String = atom.iter().rev().map(|b| format!("{b:02x}")).collect();
    let significant = digits.trim_start_matches('0').len();
    digits.drain(..digits.len() - significant);
    if digits.is_empty() {
        digits.push('0');
    }
    format!("0x{}", group(&digits, 4))
}

/// `(scow %uv n)`: `0v` and base-32 (`0-9a-v`), with a `.` every five digits.
pub fn uv(atom: &[u8]) -> String {
    const DIGITS: &[u8; 32] = b"0123456789abcdefghijklmnopqrstuv";
    let atom = trim(atom);
    let bits = atom.len() * 8;
    let mut digits = Vec::new();
    let mut i = 0;
    while i < bits {
        let mut v = 0u8;
        for k in 0..5 {
            let bit = i + k;
            if bit < bits && (atom[bit / 8] >> (bit % 8)) & 1 == 1 {
                v |= 1 << k;
            }
        }
        digits.push(DIGITS[v as usize]);
        i += 5;
    }
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    if digits.is_empty() {
        digits.push(b'0');
    }
    digits.reverse();
    format!("0v{}", group(std::str::from_utf8(&digits).unwrap(), 5))
}

/// Insert a `.` every `width` digits, counting from the right.
fn group(digits: &str, width: usize) -> String {
    let mut out = String::with_capacity(digits.len() + digits.len() / width);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(width) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

const PREFIXES: &str = "dozmarbinwansamlitsighidfidlissogdirwacsabwissib\
rigsoldopmodfoglidhopdardorlorhodfolrintogsilmir\
holpaslacrovlivdalsatlibtabhanticpidtorbolfosdot\
losdilforpilramtirwintadbicdifrocwidbisdasmidlop\
rilnardapmolsanlocnovsitnidtipsicropwitnatpanmin\
ritpodmottamtolsavposnapnopsomfinfonbanmorworsip\
ronnorbotwicsocwatdolmagpicdavbidbaltimtasmallig\
sivtagpadsaldivdactansidfabtarmonranniswolmispal\
lasdismaprabtobrollatlonnodnavfignomnibpagsopral\
bilhaddocridmocpacravripfaltodtiltinhapmicfanpat\
taclabmogsimsonpinlomrictapfirhasbosbatpochactid\
havsaplindibhosdabbitbarracparloddosbortochilmac\
tomdigfilfasmithobharmighinradmashalraglagfadtop\
mophabnilnosmilfopfamdatnoldinhatnacrisfotribhoc\
nimlarfitwalrapsarnalmoslandondanladdovrivbacpol\
laptalpitnambonrostonfodponsovnocsorlavmatmipfip";

const SUFFIXES: &str = "zodnecbudwessevpersutletfulpensytdurwepserwylsun\
rypsyxdyrnuphebpeglupdepdysputlughecryttyvsydnex\
lunmeplutseppesdelsulpedtemledtulmetwenbynhexfeb\
pyldulhetmevruttylwydtepbesdexsefwycburderneppur\
rysrebdennutsubpetrulsynregtydsupsemwynrecmegnet\
secmulnymtevwebsummutnyxrextebfushepbenmuswyxsym\
selrucdecwexsyrwetdylmynmesdetbetbeltuxtugmyrpel\
syptermebsetdutdegtexsurfeltudnuxruxrenwytnubmed\
lytdusnebrumtynseglyxpunresredfunrevrefmectedrus\
bexlebduxrynnumpyxrygryxfeptyrtustyclegnemfermer\
tenlusnussyltecmexpubrymtucfyllepdebbermughuttun\
bylsudpemdevlurdefbusbeprunmelpexdytbyttyplevmyl\
wedducfurfexnulluclennerlexrupnedlecrydlydfenwel\
nydhusrelrudneshesfetdesretdunlernyrsebhulryllud\
remlysfynwerrycsugnysnyllyndyndemluxfedsedbecmun\
lyrtesmudnytbyrsenwegfyrmurtelreptegpecnelnevfes";

fn syllable(table: &str, index: u8) -> &str {
    let i = usize::from(index) * 3;
    &table[i..i + 3]
}

/// `(scow %p n)`: a ship name such as `~sampel-palnet`.
pub fn p(atom: &[u8]) -> String {
    let mut sxz = trim(atom).to_vec();
    // fein:ob scrambles atoms of up to 64 bits; comets are left alone.
    if sxz.len() <= 8 {
        let mut buf = [0u8; 8];
        buf[..sxz.len()].copy_from_slice(&sxz);
        sxz = fein(u64::from_le_bytes(buf)).to_le_bytes().to_vec();
    }
    let sxz = trim(&sxz);

    if sxz.len() <= 1 {
        return format!("~{}", syllable(SUFFIXES, sxz.first().copied().unwrap_or(0)));
    }
    let words = sxz.len().div_ceil(2);
    let mut out = String::from("~");
    for imp in (0..words).rev() {
        let lo = sxz[2 * imp];
        let hi = sxz.get(2 * imp + 1).copied().unwrap_or(0);
        out.push_str(syllable(PREFIXES, hi));
        out.push_str(syllable(SUFFIXES, lo));
        if imp > 0 {
            out.push_str(if imp % 4 == 0 { "--" } else { "-" });
        }
    }
    out
}

/// `fein:ob`: conceal the structure of planet- and moon-sized ship numbers.
fn fein(pyn: u64) -> u64 {
    if (0x1_0000..=0xffff_ffff).contains(&pyn) {
        0x1_0000 + feis(pyn - 0x1_0000)
    } else if pyn >= 0x1_0000_0000 {
        (pyn & 0xffff_ffff_0000_0000) | fein(pyn & 0xffff_ffff)
    } else {
        pyn
    }
}

const RAKU: [u32; 4] = [0xb76d_5eed, 0xee28_1300, 0x85bc_ae01, 0x4b38_7af7];

/// `feis:ob`: a four-round Feistel cipher over `[0, 0xffff * 0x10000)`,
/// cycle-walked so the result stays in range.
fn feis(m: u64) -> u64 {
    let c = fe(m);
    if c < 0xffff * 0x1_0000 { c } else { fe(c) }
}

fn fe(m: u64) -> u64 {
    const A: u64 = 0xffff;
    const B: u64 = 0x1_0000;
    const ROUNDS: u64 = 4;
    let mut ell = m % A;
    let mut arr = m / A;
    for j in 1..=ROUNDS {
        let key = [(arr & 0xff) as u8, ((arr >> 8) & 0xff) as u8];
        let f = u64::from(murmur3_32(&key, RAKU[(j - 1) as usize]));
        let tmp = if j % 2 == 1 {
            (f + ell) % A
        } else {
            (f + ell) % B
        };
        ell = arr;
        arr = tmp;
    }
    // ROUNDS is even.
    if arr == A {
        arr * A + ell
    } else {
        ell * A + arr
    }
}
