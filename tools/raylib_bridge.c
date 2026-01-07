// Minimal Raylib ABI shim for Aura.
//
// Build notes (Windows): link with raylib import library, e.g. raylib.lib
// This translation unit intentionally avoids including raylib.h to keep
// the bridge independent of include paths.

#ifdef __cplusplus
extern "C" {
#endif

// Forward declarations of the raylib C API we use.
// (Signatures match raylib's public API.)
void InitWindow(int width, int height, const char* title);
void SetTargetFPS(int fps);
_Bool WindowShouldClose(void);

void BeginDrawing(void);
void EndDrawing(void);
void CloseWindow(void);

typedef struct Color {
    unsigned char r;
    unsigned char g;
    unsigned char b;
    unsigned char a;
} Color;

void ClearBackground(Color color);
void DrawPixel(int posX, int posY, Color color);
void DrawText(const char* text, int posX, int posY, int fontSize, Color color);

// Aura-friendly wrappers.
void raylib_init_window(int width, int height, const char* title) { InitWindow(width, height, title); }
void raylib_set_target_fps(int fps) { SetTargetFPS(fps); }
unsigned int raylib_window_should_close(void) { return WindowShouldClose() ? 1u : 0u; }

void raylib_begin_drawing(void) { BeginDrawing(); }
void raylib_end_drawing(void) { EndDrawing(); }
void raylib_close_window(void) { CloseWindow(); }

static Color mk_color(unsigned int r, unsigned int g, unsigned int b, unsigned int a) {
    Color c;
    c.r = (unsigned char)(r & 0xFFu);
    c.g = (unsigned char)(g & 0xFFu);
    c.b = (unsigned char)(b & 0xFFu);
    c.a = (unsigned char)(a & 0xFFu);
    return c;
}

void raylib_clear_background_rgba(unsigned int r, unsigned int g, unsigned int b, unsigned int a) {
    ClearBackground(mk_color(r, g, b, a));
}

void raylib_draw_pixel_rgba(int x, int y, unsigned int r, unsigned int g, unsigned int b, unsigned int a) {
    DrawPixel(x, y, mk_color(r, g, b, a));
}

void raylib_draw_text_rgba(const char* text, int x, int y, int font_size, unsigned int r, unsigned int g, unsigned int b, unsigned int a) {
    DrawText(text, x, y, font_size, mk_color(r, g, b, a));
}

#ifdef __cplusplus
}
#endif
