# Python script to generate Rust code for swizzle methods

def generate_swizzle_code():
    components = ['x', 'y', 'z', 'w']
    index_map = {'x': 0, 'y': 1, 'z': 2, 'w': 3}

    # List of method names to generate code for
    method_names = [
        "xxxx", "yxxx", "zxxx",
        "wxxx", "xyxx", "yyxx", "zyxx", "wyxx", "xzxx", "yzxx", "zzxx", "wzxx", "xwxx", "ywxx", "zwxx", "wwxx",
        "xxyx", "yxyx", "zxyx", "wxyx", "xyyx", "yyyx", "zyyx", "wyyx", "xzyx", "yzyx", "zzyx", "wzyx", "xwyx",
        "ywyx", "zwyx", "wwyx", "xxzx", "yxzx", "zxzx", "wxzx", "xyzx", "yyzx", "zyzx", "wyzx", "xzzx", "yzzx",
        "zzzx", "wzzx", "xwzx", "ywzx", "zwzx", "wwzx", "xxwx", "yxwx", "zxwx", "wxwx", "xywx", "yywx", "zywx",
        "wywx", "xzwx", "yzwx", "zzwx", "wzwx", "xwwx", "ywwx", "zwwx", "wwwx", "xxxy", "yxxy", "zxxy", "wxxy",
        "xyxy", "yyxy", "zyxy", "wyxy", "xzxy", "yzxy", "zzxy", "wzxy", "xwxy", "ywxy", "zwxy", "wwxy", "xxyy",
        "yxyy", "zxyy", "wxyy", "xyyy", "yyyy", "zyyy", "wyyy", "xzyy", "yzyy", "zzyy", "wzyy", "xwyy", "ywyy",
        "zwyy", "wwyy", "xxzy", "yxzy", "zxzy", "wxzy", "xyzy", "yyzy", "zyzy", "wyzy", "xzzy", "yzzy", "zzzy",
        "wzzy", "xwzy", "ywzy", "zwzy", "wwzy", "xxwy", "yxwy", "zxwy", "wxwy", "xywy", "yywy", "zywy", "wywy",
        "xzwy", "yzwy", "zzwy", "wzwy", "xwwy", "ywwy", "zwwy", "wwwy", "xxxz", "yxxz", "zxxz", "wxxz", "xyxz",
        "yyxz", "zyxz", "wyxz", "xzxz", "yzxz", "zzxz", "wzxz", "xwxz", "ywxz", "zwxz", "wwxz", "xxyz", "yxyz",
        "zxyz", "wxyz", "xyyz", "yyyz", "zyyz", "wyyz", "xzyz", "yzyz", "zzyz", "wzyz", "xwyz", "ywyz", "zwyz",
        "wwyz", "xxzz", "yxzz", "zxzz", "wxzz", "xyzz", "yyzz", "zyzz", "wyzz", "xzzz", "yzzz", "zzzz", "wzzz",
        "xwzz", "ywzz", "zwzz", "wwzz", "xxwz", "yxwz", "zxwz", "wxwz", "xywz", "yywz", "zywz", "wywz", "xzwz",
        "yzwz", "zzwz", "wzwz", "xwwz", "ywwz", "zwwz", "wwwz", "xxxw", "yxxw", "zxxw", "wxxw", "xyxw", "yyxw",
        "zyxw", "wyxw", "xzxw", "yzxw", "zzxw", "wzxw", "xwxw", "ywxw", "zwxw", "wwxw", "xxyw", "yxyw", "zxyw",
        "wxyw", "xyyw", "yyyw", "zyyw", "wyyw", "xzyw", "yzyw", "zzyw", "wzyw", "xwyw", "ywyw", "zwyw", "wwyw",
        "xxzw", "yxzw", "zxzw", "wxzw", "xyzw", "yyzw", "zyzw", "wyzw", "xzzw", "yzzw", "zzzw", "wzzw", "xwzw",
        "ywzw", "zwzw", "wwzw", "xxww", "yxww", "zxww", "wxww", "xyww", "yyww", "zyww", "wyww", "xzww", "yzww",
        "zzww", "wzww", "xwww", "ywww", "zwww", "wwww"
    ]

    for method in method_names:
        indices = [index_map[char] for char in method]
        print(f"    #[inline]")
        print(f"    #[cfg(target_arch = \"wasm32\")]")
        print(f"    #[target_feature(enable = \"simd128\")]")
        print(f"    pub fn {method}(self) -> F32x4 {{")
        print(f"        F32x4(std::arch::wasm32::i32x4_shuffle::<{indices[0]}, {indices[1]}, {indices[2]}, {indices[3]}>(self.0, self.0))")
        print(f"    }}\n")

generate_swizzle_code()
