// Minimal Raylib ABI shim for Aura (bootstrap)
// This header is designed to be parsed by aura-bridge's regex-based extractor.
// Keep prototypes simple (no structs in the signature).

#ifdef __cplusplus
extern "C" {
#endif

void raylib_init_window(int width, int height, const char* title);
void raylib_set_target_fps(int fps);
unsigned int raylib_window_should_close(void);

void raylib_begin_drawing(void);
void raylib_end_drawing(void);
void raylib_close_window(void);

void raylib_clear_background_rgba(unsigned int r, unsigned int g, unsigned int b, unsigned int a);
void raylib_draw_pixel_rgba(int x, int y, unsigned int r, unsigned int g, unsigned int b, unsigned int a);
void raylib_draw_text_rgba(const char* text, int x, int y, int font_size, unsigned int r, unsigned int g, unsigned int b, unsigned int a);

#ifdef __cplusplus
}
#endif
