-- The kinds of symbol listed, so the list outlines the types and functions without their variables
-- Each has a tag and a color for it, both as aerial.nvim shows them without an icon font
-- Ref: https://github.com/stevearc/aerial.nvim/blob/28fe6e822ae344544c379d60fcb13c9519a1f08a/lua/aerial/highlight.lua
local kinds = {
    Class = { tag = '[C]', hl_group = 'Type' },
    Constructor = { tag = '[Co]', hl_group = 'Special' },
    Enum = { tag = '[E]', hl_group = 'Type' },
    Function = { tag = '[F]', hl_group = 'Function' },
    Interface = { tag = '[I]', hl_group = 'Type' },
    Method = { tag = '[M]', hl_group = 'Function' },
    Module = { tag = '[Mod]', hl_group = 'Include' },
    Struct = { tag = '[S]', hl_group = 'Type' },
}

local tags_namespace = vim.api.nvim_create_namespace('symbol-tags')

local function can_list_symbols(buf)
    return vim.api.nvim_buf_is_valid(buf)
        and #vim.lsp.get_clients({ bufnr = buf, method = 'textDocument/documentSymbol' }) > 0
end

-- The symbols of a buffer in the order they are listed, each with its depth and the position of its name
local function get_symbols(source_buf)
    local params = { textDocument = vim.lsp.util.make_text_document_params(source_buf) }
    -- A server still silent after a second is busy, as while it indexes, and is better asked again than waited on
    local responses = assert(vim.lsp.buf_request_sync(source_buf, 'textDocument/documentSymbol', params, 1000))
    local symbols = {}
    for client_id, response in pairs(responses) do
        if response.err ~= nil then error(response.err.message) end
        local encoding = vim.lsp.get_client_by_id(client_id).offset_encoding
        -- nvim asks for each symbol nested in its parent, rather than for a flat list
        local function add_symbols(document_symbols, depth)
            for _, symbol in ipairs(document_symbols) do
                local kind = kinds[vim.lsp.protocol.SymbolKind[symbol.kind]]
                -- A symbol of another kind is left out, but what it holds is still listed, like the methods of a Rust impl
                if kind == nil then
                    add_symbols(symbol.children or {}, depth)
                else
                    local start = symbol.selectionRange.start
                    local line = vim.api.nvim_buf_get_lines(source_buf, start.line, start.line + 1, false)[1]
                    table.insert(symbols, {
                        kind = kind,
                        name = symbol.name,
                        depth = depth,
                        line = start.line + 1,
                        -- A server counts columns in its own encoding, and nvim counts them in bytes
                        column = vim.str_byteindex(line, encoding, start.character, false),
                    })
                    add_symbols(symbol.children or {}, depth + 1)
                end
            end
        end
        add_symbols(response.result or {}, 0)
    end
    return symbols
end

local function fill_symbols_buf(symbols_buf)
    local source_buf = vim.b[symbols_buf].symbols_source
    -- A file that was closed, or that lost its server, leaves the list as it last was
    if not can_list_symbols(source_buf) then return end
    local symbols = get_symbols(source_buf)
    local lines = {}
    local tag_starts = {}
    for row, symbol in ipairs(symbols) do
        local indent = string.rep('  ', symbol.depth)
        lines[row] = indent .. symbol.kind.tag .. ' ' .. symbol.name
        tag_starts[row] = #indent
    end
    vim.bo[symbols_buf].modifiable = true
    vim.api.nvim_buf_set_lines(symbols_buf, 0, -1, false, lines)
    vim.bo[symbols_buf].modifiable = false
    vim.api.nvim_buf_clear_namespace(symbols_buf, tags_namespace, 0, -1)
    for row, symbol in ipairs(symbols) do
        vim.api.nvim_buf_set_extmark(symbols_buf, tags_namespace, row - 1, tag_starts[row], {
            end_col = tag_starts[row] + #symbol.kind.tag,
            hl_group = symbol.kind.hl_group,
        })
    end
    vim.b[symbols_buf].symbol_positions = vim.tbl_map(
        function(symbol) return { symbol.line, symbol.column } end,
        symbols
    )
end

-- Goes from a symbols buffer to the name of the symbol under the cursor
local function jump_to_symbol()
    local position = vim.b.symbol_positions[vim.api.nvim_win_get_cursor(0)[1]]
    if position == nil then return end
    vim.cmd.buffer(vim.b.symbols_source)
    vim.api.nvim_win_set_cursor(0, position)
end

-- Shows the symbols of the current buffer in its place, with the cursor on the symbol the cursor was in
local function open_symbols()
    local source_buf = vim.api.nvim_get_current_buf()
    -- Checked before switching, so a file with no server stays in the window instead of an empty list
    if not can_list_symbols(source_buf) then
        error('No language server lists the symbols of ' .. vim.api.nvim_buf_get_name(source_buf))
    end
    local cursor_line = vim.api.nvim_win_get_cursor(0)[1]
    local symbols_buf = vim.fn.bufadd('symbols:' .. vim.fn.expand('%:.'))
    -- Set on each open, since the file may be in a new buffer after it was closed
    vim.b[symbols_buf].symbols_source = source_buf
    -- Only a buffer bufadd just made still has no buftype
    if vim.bo[symbols_buf].buftype == '' then
        vim.bo[symbols_buf].buftype = 'nofile'
        -- Kept when hidden, so CTRL-O and CTRL-I can come back to it
        vim.bo[symbols_buf].bufhidden = 'hide'
        -- Filled each time it is entered, so it never lists symbols the file no longer has
        vim.api.nvim_create_autocmd('BufEnter', {
            buffer = symbols_buf,
            callback = function()
                -- The cursor's line stands out as in netrw, local to this buffer in this window, so the file shown next keeps its own
                vim.wo[0][0].cursorline = true
                fill_symbols_buf(symbols_buf)
            end,
        })
        vim.keymap.set('n', '<CR>', jump_to_symbol, { buffer = symbols_buf })
    end
    vim.cmd.buffer(symbols_buf)
    -- The cursor lands on the last symbol starting at or above the line it left, which is the symbol it was in or the one before
    local row = 1
    for index, position in ipairs(vim.b[symbols_buf].symbol_positions) do
        if position[1] <= cursor_line then row = index end
    end
    vim.api.nvim_win_set_cursor(0, { row, 0 })
end

return { open = open_symbols }
