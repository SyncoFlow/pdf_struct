#include <mupdf/fitz.h>
#include <stdexcept>
#include <format>
#include <mutex>
#include <queue>
#include <memory>
#include <thread>
#include <fstream>
#include "main.h"

#define SCALE 6.0f // 432 DPI
#define THRESHOLD 128.5f

// Global mutexes for MuPDF locking - shared across all contexts
static std::mutex fz_mutexes[FZ_LOCK_MAX];

// Additional mutex for context creation to prevent race conditions
static std::mutex context_creation_mutex;

// Thread-local storage for frequently accessed resources
thread_local fz_matrix cached_ctm = fz_scale(SCALE, SCALE);

// Context pool for better memory management
struct ContextPool
{
    std::queue<fz_context *> available_contexts;
    std::mutex pool_mutex;
    size_t max_pool_size;

    ContextPool(size_t max_size = 32) : max_pool_size(max_size) {}

    fz_context *get_context()
    {
        std::lock_guard<std::mutex> lock(pool_mutex);
        if (!available_contexts.empty())
        {
            auto ctx = available_contexts.front();
            available_contexts.pop();
            return ctx;
        }
        return nullptr; // Will create new one if pool is empty
    }

    void return_context(fz_context *ctx)
    {
        if (!ctx)
            return;
        std::lock_guard<std::mutex> lock(pool_mutex);
        if (available_contexts.size() < max_pool_size)
        {
            // Clear the context store to prevent memory bloat
            fz_empty_store(ctx);
            available_contexts.push(ctx);
        }
        else
        {
            // Pool is full, just drop the context
            fz_drop_context(ctx);
        }
    }

    ~ContextPool()
    {
        std::lock_guard<std::mutex> lock(pool_mutex);
        while (!available_contexts.empty())
        {
            fz_drop_context(available_contexts.front());
            available_contexts.pop();
        }
    }
};

static ContextPool global_context_pool;

static void lock_mutex(void *user, int lock)
{
    if (lock >= 0 && lock < FZ_LOCK_MAX)
    {
        fz_mutexes[lock].lock();
    }
}

static void unlock_mutex(void *user, int lock)
{
    if (lock >= 0 && lock < FZ_LOCK_MAX)
    {
        fz_mutexes[lock].unlock();
    }
}

void init(const std::string &path, PDFHandle *doc_handle, PDFHandle *ctx_handle, int *pages_buf)
{
    _setmaxstdio(8192);

    // Create context with larger memory allocation for big files
    // Check file size to determine appropriate context size
    std::ifstream file(path, std::ifstream::ate | std::ifstream::binary);
    size_t file_size = 0;
    if (file.is_open())
    {
        file_size = file.tellg();
        file.close();
    }

    // Scale memory allocation based on file size
    size_t context_memory = 256 << 20; // Default 256MB
    if (file_size > 100 << 20)
    {                               // Files larger than 100MB
        context_memory = 512 << 20; // Use 512MB
    }
    if (file_size > 500 << 20)
    {                                // Files larger than 500MB
        context_memory = 1024 << 20; // Use 1GB
    }

    // Create context with custom locking for multi-threading
    fz_locks_context locks;
    locks.user = nullptr;
    locks.lock = lock_mutex;
    locks.unlock = unlock_mutex;

    fz_context *ctx = fz_new_context(nullptr, &locks, context_memory);
    if (!ctx)
    {
        throw std::runtime_error("Failed to create Context!");
    }

    fz_set_aa_level(ctx, 8);

    fz_try(ctx)
    {
        fz_register_document_handlers(ctx);
    }
    fz_catch(ctx)
    {
        fz_drop_context(ctx);
        throw std::runtime_error("Failed to register document handlers!");
    }

    fz_document *doc = nullptr;
    fz_try(ctx)
    {
        doc = fz_open_document(ctx, path.c_str());
    }
    fz_catch(ctx)
    {
        fz_drop_context(ctx);
        throw std::runtime_error("Failed to open document at path " + path);
    }

    if (!doc)
    {
        fz_drop_context(ctx);
        throw std::runtime_error("Document was null after being opened!");
    }

    int page_count = 0;
    fz_try(ctx)
    {
        page_count = fz_count_pages(ctx, doc);
    }
    fz_catch(ctx)
    {
        fz_drop_document(ctx, doc);
        fz_drop_context(ctx);

        throw std::runtime_error("Failed to count pages of document.");
    }

    *doc_handle = (PDFHandle)doc;
    *ctx_handle = (PDFHandle)ctx;
    *pages_buf = page_count;

    // Debug output
    // printf("[DEBUG] init() completed:\n");
    // printf("  doc=%p -> *doc_handle=%p\n", doc, *doc_handle);
    // printf("  ctx=%p -> *ctx_handle=%p\n", ctx, *ctx_handle);
    // printf("  page_count=%d\n", page_count);
    // printf("  file_size=%zuMB, context_memory=%zuMB\n", file_size >> 20, context_memory >> 20);
}

uint8_t *render_page(int page_num, size_t *size_buf, int *width_buf, int *height_buf,
                     int *channels_buf, PDFHandle *doc_handle, PDFHandle *ctx_handle)
{
    // printf("[DEBUG] render_page() called:\n  page_num=%d\n  ctx_handle=%p\n  *ctx_handle=%p\n  doc_handle=%p\n  *doc_handle=%p\n",
    //        page_num, ctx_handle, ctx_handle ? *ctx_handle : nullptr, doc_handle, doc_handle ? *doc_handle : nullptr);

    if (!ctx_handle || !*ctx_handle)
    {
        // printf("[DEBUG] Invalid context handle detected!\n");
        throw std::runtime_error("Invalid context handle");
    }

    if (!doc_handle || !*doc_handle)
    {
        // printf("[DEBUG] Invalid document handle detected!\n");
        throw std::runtime_error("Invalid document handle");
    }

    if (size_buf == nullptr || width_buf == nullptr || height_buf == nullptr || channels_buf == nullptr)
    {
        throw std::runtime_error("Passed nullptr for a buffer!");
    }

    fz_context *ctx = (fz_context *)(*ctx_handle);
    fz_document *doc = (fz_document *)(*doc_handle);

    // printf("[DEBUG] Converted pointers: ctx=%p, doc=%p\n", ctx, doc);

    // Additional safety checks for potentially corrupted documents
    if (!ctx || !doc)
    {
        throw std::runtime_error("Invalid context or document handle");
    }

    // Validate page number with proper error handling for potentially corrupted documents
    int total_pages = 0;
    fz_try(ctx)
    {
        total_pages = fz_count_pages(ctx, doc);
        // Additional check for document validity
        if (total_pages <= 0)
        {
            throw std::runtime_error("Document appears to be corrupted - no valid pages found");
        }

        // Extra validation for page access
        if (page_num < 0 || page_num >= total_pages)
        {
            throw std::runtime_error(std::format("Attempted to access page {} but document only has {} pages!", page_num, total_pages));
        }
    }
    fz_catch(ctx)
    {
        const char *msg = fz_caught_message(ctx);
        // Handle specific document corruption cases
        if (msg && (strstr(msg, "object out of range") || strstr(msg, "page tree")))
        {
            throw std::runtime_error("Document is corrupted and cannot be processed in multi-threaded mode");
        }
        throw std::runtime_error(std::format("Failed to count pages or document corrupted: {}", msg ? msg : "Unknown error"));
    }

    if (page_num < 0 || page_num >= total_pages)
    {
        // This check is now handled above in the fz_try block
        // But keeping this as a fallback
        throw std::runtime_error(std::format("Attempted to access page {} but document only has {} pages!", page_num, total_pages));
    }

    fz_page *page = nullptr;
    fz_pixmap *gray_pix = nullptr;
    fz_pixmap *bilevel_pix = nullptr;
    fz_buffer *png_buffer = nullptr;
    uint8_t *data = nullptr;

    fz_try(ctx)
    {
        page = fz_load_page(ctx, doc, page_num);
        if (!page)
        {
            throw std::runtime_error(std::format("Failed to load page {}", page_num));
        }

        // Use thread-local cached matrix instead of creating new one each time
        gray_pix = fz_new_pixmap_from_page(ctx, page, cached_ctm, fz_device_gray(ctx), 0);

        if (!gray_pix)
        {
            const char *error_msg = fz_caught_message(ctx);
            if (error_msg)
            {
                throw std::runtime_error(std::format("Failed to create pixmap: {}", error_msg));
            }
            else
            {
                throw std::runtime_error("Failed to create pixmap: Unknown error");
            }
        }

        int width = fz_pixmap_width(ctx, gray_pix);
        int height = fz_pixmap_height(ctx, gray_pix);

        bilevel_pix = fz_new_pixmap(ctx, fz_device_gray(ctx), width, height, NULL, 0);
        if (!bilevel_pix)
        {
            throw std::runtime_error("Failed to create bilevel pixmap");
        }

        unsigned char *gray_samples = fz_pixmap_samples(ctx, gray_pix);
        unsigned char *bilevel_samples = fz_pixmap_samples(ctx, bilevel_pix);

        // Add prefetching hints for better cache performance
        const int total_pixels = width * height;

        for (int i = 0; i < total_pixels; i++)
        {
            bilevel_samples[i] = (gray_samples[i] > THRESHOLD) ? 255 : 0;
        }

        png_buffer = fz_new_buffer_from_pixmap_as_png(ctx, bilevel_pix, fz_default_color_params);
        if (!png_buffer)
        {
            throw std::runtime_error("Failed to create PNG buffer");
        }

        // Use zero-copy approach - get direct pointer to buffer data
        size_t png_size = fz_buffer_storage(ctx, png_buffer, &data);

        if (!data || png_size == 0)
        {
            throw std::runtime_error("Failed to get PNG buffer storage");
        }

        // Allocate and copy only once
        uint8_t *result_data = new uint8_t[png_size];
        memcpy(result_data, data, png_size);
        data = result_data;

        int channels = fz_pixmap_components(ctx, bilevel_pix);

        *size_buf = png_size;
        *width_buf = width;
        *height_buf = height;
        *channels_buf = channels;
    }
    fz_always(ctx)
    {
        if (png_buffer)
            fz_drop_buffer(ctx, png_buffer);
        if (bilevel_pix)
            fz_drop_pixmap(ctx, bilevel_pix);
        if (gray_pix)
            fz_drop_pixmap(ctx, gray_pix);
        if (page)
            fz_drop_page(ctx, page);
    }
    fz_catch(ctx)
    {
        if (data)
        {
            delete[] data;
            data = nullptr;
        }
        const char *msg = fz_caught_message(ctx);
        throw std::runtime_error(std::format("Failed to render page {}: {}", page_num, msg ? msg : "Unknown error"));
    }

    return data;
}

void free_image_data(uint8_t *data)
{
    delete[] data;
}

void cleanup_pdf(PDFHandle *doc_handle, PDFHandle *ctx_handle)
{
    printf("[DEBUG] cleanup_pdf() called with doc_handle=%p, ctx_handle=%p\n",
           doc_handle ? *doc_handle : nullptr, ctx_handle ? *ctx_handle : nullptr);

    // Clean up document first if it exists
    if (doc_handle && *doc_handle)
    {
        fz_context *ctx = (fz_context *)(*ctx_handle);
        fz_document *doc = (fz_document *)(*doc_handle);

        if (ctx && doc)
        {
            fz_try(ctx)
            {
                fz_drop_document(ctx, doc);
            }
            fz_catch(ctx)
            {
                printf("[DEBUG] Error while dropping document: %s\n", fz_caught_message(ctx));
            }
        }
        *doc_handle = nullptr;
        printf("[DEBUG] Document handle cleaned up\n");
    }

    // Return context to pool instead of dropping it immediately
    if (ctx_handle && *ctx_handle)
    {
        fz_context *ctx = (fz_context *)(*ctx_handle);

        if (ctx)
        {
            // Return to pool instead of dropping
            global_context_pool.return_context(ctx);
            printf("[DEBUG] Context returned to pool\n");
        }
        *ctx_handle = nullptr;
        printf("[DEBUG] Context handle cleaned up\n");
    }
}

void flush_cache(PDFHandle *ctx_handle)
{
    if (ctx_handle && *ctx_handle)
    {
        fz_context *ctx = (fz_context *)(*ctx_handle);

        fz_empty_store(ctx);

        fz_shrink_store(ctx, 0);
    }
}

void clone(PDFHandle *current_ctx, PDFHandle *new_ctx)
{
    if (current_ctx == nullptr || new_ctx == nullptr)
    {
        throw std::runtime_error("Passed a nullptr when trying to clone context!");
    }

    printf("[DEBUG] clone() called:\n  current_ctx=%p\n  *current_ctx=%p\n", current_ctx, *current_ctx);

    // Try to get a context from the pool first
    fz_context *pooled_context = global_context_pool.get_context();

    if (pooled_context)
    {
        printf("[DEBUG] clone() using pooled context: %p\n", pooled_context);
        *new_ctx = (PDFHandle)pooled_context;
        return;
    }

    // Protect context creation with mutex to prevent race conditions
    std::lock_guard<std::mutex> lock(context_creation_mutex);

    // Create a completely new context with the same locking mechanism
    // instead of cloning to avoid shared resource issues
    fz_locks_context locks;
    locks.user = nullptr;
    locks.lock = lock_mutex;
    locks.unlock = unlock_mutex;

    fz_context *new_context = fz_new_context(nullptr, &locks, 256 << 20);
    if (new_context == nullptr)
    {
        throw std::runtime_error("Failed to create new context!");
    }

    fz_set_aa_level(new_context, 8);

    fz_try(new_context)
    {
        fz_register_document_handlers(new_context);
    }
    fz_catch(new_context)
    {
        fz_drop_context(new_context);
        throw std::runtime_error("Failed to register document handlers in new context!");
    }

    printf("[DEBUG] clone() completed: new_context=%p -> *new_ctx=%p\n", new_context, new_context);
    *new_ctx = (PDFHandle)new_context;
}

void clone_doc(const std::string &path, PDFHandle *ctx_handle, PDFHandle *new_doc)
{
    if (ctx_handle == nullptr || new_doc == nullptr)
    {
        throw std::runtime_error("Passed a nullptr when trying to clone document!");
    }

    fz_context *ctx = (fz_context *)(*ctx_handle);

    // Open the document fresh for this thread
    fz_document *doc = nullptr;
    fz_try(ctx)
    {
        doc = fz_open_document(ctx, path.c_str());
        if (!doc)
        {
            throw std::runtime_error("Failed to open document for cloning");
        }

        // Validate the document immediately
        int test_pages = fz_count_pages(ctx, doc);
        if (test_pages <= 0)
        {
            fz_drop_document(ctx, doc);
            throw std::runtime_error("Document has no valid pages");
        }
    }
    fz_catch(ctx)
    {
        if (doc)
        {
            fz_drop_document(ctx, doc);
        }
        throw std::runtime_error(std::format("Failed to clone document: {}", fz_caught_message(ctx)));
    }

    *new_doc = (PDFHandle)doc;
}