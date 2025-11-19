/**
 * - maybe the reason why each start is so similar, except for the first character, is that it says:
 *   - "1. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "2. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "3. ABCDEFGHIJKLMNOPQRSTUV..."
 *   - "4. WXYZ..."
 *   - "5. WXYZABCD..."
 *   - "6. WXYZ..."
 *   - "7. WXYZABCDEFGHIJK..."
 *   - "8. WXYZABCDEFGHIJK..."
 *   - "9. WXYZABCDEFGHIJK..."
 */

// e1, w1, ..., e4, w4, e5
const BASE10_PLUS_32: &'static [&'static [u8]] = &[
    b"Rb%P^-k=8]Jfb^@.q(/n\"=-Q!prH_q53 HSa:.5fOLPJ3P-O3Qh?%8#K[cAQI\\5:>%94g+jX$j3g$SIKphV_oq/0L?>,AY<-`KP",
    b"pb%P^-k=8]Jfb^@.q(/n\"=-Q!=+>Tq53 9:V4.5fOLPJ3P-O3QL:[m`Ko<h`!>i7c&A9`qdN1D-15d-)NcYB^r/*i^\"+ahEL*Kd^)B2",
    b"Db%P^-k=8]Jfb^@.q(/n\"=-Q!elT)Pbp6`YHQn#0X3OHp&-`=Q`_&Q?-0*M8:m*\\q]BVf5/$bmJE>6 +IhY47YaI72hJ%#:n(%VMm9`]0LVS4_9+:MU\\FB",
    b"lb%QkVeN@!J\\:PRp@8W]O,5,QVB9D/XW4)(^-r)L=\\UrJp%Kg#pmOnB9^2*Q^`Tq+b^-O1Tf:7@?`7C@R&!9(EOK:ladp1'M_.U_\\0",
    b"_b%QkV\"\\=HnO\\kcg\\\"a'O.Mj[Ip-\\-q6CRHG\"[P?l\"pk!Xc+5(HaMkWG\\J-#6Y\"&Z)f!ZX_d9o'43`\"bi>g0,>aE4-6_2N`[Iqr6nDO1$&1%Do_!`e/K$ZX?.`Z2Lne! N4gi9C(8",
    b"Bb%QkV7j+-<:3PcYE\\B<j*1@+23K3qJ$^)NQ@SlZ$KO1co5@L0>E:<IdYBS*ef(&NK2GOK/-A>C^E E%FWE-H9)5+`%oJd+g+P#c]H6.CR]G+\"bQSU1iDkjV8>Vf",
    b";b%QkV\"\\=H\"W)/[2d#D%OmLF!2<l$B\\_Zp1VokPVW3^`.OSfk%+OMZdeo9FMiOdRBMn:oY$X6\\2kK\\[c_JQAHaom'#:^?n:YeH$7:-cJFh+Ga\\9&pbdm[n3",
    b"mb%QkV\"\\=H\"W)/[2d#D%O\\5p!hW0rCY3!b2;G1jqG.n 9aKb`Fq78RY>gk:dVYXRgi.5(@:_%E3KbOUBb7i?VFmc+_o&65Sej5%1cE=5\\.rL>$4JC!?VN4H>",
    b"Ab%QkV\"\\=H\"W)/[2d#D%OA5[L2<l[B\\_o;,V%QPVWT^he*Y6ZPcU'B@>?3:(BN'>gWBkV)&\\%79MJp9,6l4S^5H)I*Li(Afi&?5h%H]SJb`j]9_J8I",
];

fn sub32_message(msg: &[u8]) -> Vec<u8> {
    let mut m_out: Vec<u8> = Vec::new();

    for c in msg {
        m_out.push(c.wrapping_add_signed(-32));
    }

    m_out
}

pub fn gen_message_structs(msg_len_min: usize) {
    let mut max_len = msg_len_min;
    for data in BASE10_PLUS_32 {
        let len = data.len();
        if len > max_len { max_len = len };
    }

    println!("\
/**
 * AUTO-GENERATED FILE - DO NOT MODIFY.
 * Use cargo run -- --codegen instead, and paste the output into this file.
 */

pub struct Message {{
    pub name: &'static str,
    pub data_len: usize,
    // zero-padded array, so that all messages have the same size
    pub data: [u8; {}],
}}

pub type MessageList = [Message; 9];

pub const MESSAGES: MessageList = [\
"   , max_len);

    for m in 0..BASE10_PLUS_32.len() {
        let data = &sub32_message(BASE10_PLUS_32[m]);
        let name = format!("{}-{}", if m % 2 == 0 { "east" } else { "west" }, m / 2 + 1);

        let mut first = true;
        print!("    Message {{\n        name: \"{}\",\n        data_len: {},\n        data: [", name, data.len());
        for c in data {
            if first {
                first = false;
            } else {
                print!(",");
            }
            print!("{}", c);
        }

        let pad_amount = max_len - data.len();
        first = true;
        if pad_amount > 0 {
            print!(", /* {}-byte padding */ ", pad_amount);
            for _ in 0..pad_amount {
                if first {
                    first = false;
                    print!("0");
                } else {
                    print!(",0");
                }
            }
        }

        println!("],\n    }},");
    }
    println!("];");
}