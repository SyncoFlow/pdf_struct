#pragma once

#include <cstdint>
#include <string>

typedef void *PDFHandle;

// All arguments for the following functions that are a pointer to any type
// Are just "buffers" for data. Since CXX is unbelievably annoying with structs and
// other complex types, it's easier to just expect buffers.

void init(const std::string &path, PDFHandle *doc_handle, PDFHandle *ctx_handle, int *pages_buf);
uint8_t *render_page(int page_num, size_t *size_buf, int *width_buf, int *height_buf,
                     int *channels_buf, PDFHandle *doc_handle, PDFHandle *ctx_handle);
void free_image_data(uint8_t *data);
void cleanup_pdf(PDFHandle *doc_handle, PDFHandle *ctx_handle);
void flush_cache(PDFHandle *ctx_handle);
void clone(PDFHandle *current_ctx, PDFHandle *new_ctx);
void clone_doc(const std::string &path, PDFHandle *ctx_handle, PDFHandle *new_doc);