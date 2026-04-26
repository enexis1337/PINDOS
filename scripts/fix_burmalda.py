data = open('kernel/src/mell/apps/burmalda.rs', 'rb').read()
# Находим строку с b'<newline>' и заменяем на 0x24u8 (символ $)
idx = data.find(b"put_char_at_bg(cx, input_y, b'")
while idx != -1:
    end = data.find(b"', WHITE, BLACK); cx += 1;", idx)
    if end != -1:
        char_byte = data[idx + len(b"put_char_at_bg(cx, input_y, b'")]
        if char_byte == 0x0A:  # newline вместо $
            print(f"Found broken literal at {idx}, char byte = 0x{char_byte:02x}")
            bad = data[idx:end + len(b"', WHITE, BLACK); cx += 1;")]
            good = b"put_char_at_bg(cx, input_y, 0x24u8, WHITE, BLACK); cx += 1;"
            data = data.replace(bad, good)
            print("Fixed!")
            break
    idx = data.find(b"put_char_at_bg(cx, input_y, b'", idx + 1)

open('kernel/src/mell/apps/burmalda.rs', 'wb').write(data)
print("Done")
