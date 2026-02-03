// Lua 5.4 Standard Library Type Definitions
// This file contains type definitions for Lua 5.4 built-in functions and libraries

// Global functions
declare function print(...: any): void
declare function error(message: string, level?: number): never
declare function assert<T>(value: T, message?: string): T
declare function collectgarbage(opt: string, arg?: number): number
declare function dofile(filename?: string): any
declare function getmetatable(object: any): any
declare function ipairs<T>(t: { [number]: T }): (() => (number, T)?)
declare function load(chunk: string | (() => string?), chunkname?: string, mode?: string, env?: any): (() => any)?, string?
declare function loadfile(filename?: string, mode?: string, env?: any): (() => any)?, string?
declare function next(table: any, index?: any): (any, any)?
declare function pairs<T>(t: { [string]: T }): (() => (string, T)?)
declare function pcall<T>(f: () => T): boolean, T | string
declare function rawequal(v1: any, v2: any): boolean
declare function rawget(table: any, index: any): any
declare function rawlen(v: any): number
declare function rawset(table: any, index: any, value: any): any
declare function require(modname: string): any
declare function select(index: number | string, ...: any): any
declare function setmetatable(table: any, metatable: any): any
declare function tonumber(e: any, base?: number): number?
declare function tostring(v: any): string
declare function type(v: any): string
declare function warn(...: any): void
declare function xpcall<T>(f: () => T, msgh: (string) => any): boolean, T | string

// Coroutine namespace
declare namespace coroutine {
    function create(f: () => any): thread
    function close(co: thread): boolean, string?
    function isyieldable(): boolean
    function resume(co: thread, ...: any): boolean, ...: any
    function running(): thread, boolean
    function status(co: thread): string
    function wrap(f: () => any): () => ...: any
    function yield(...: any): ...: any
}

// Debug namespace
declare namespace debug {
    function traceback(thread?: thread, message?: string, level?: number): string
    function getinfo(thread?: thread, f: any, what?: string): any
    function getlocal(thread?: thread, f: any, local: number): (string, any)?
    function setlocal(thread?: thread, f: any, local: number, value: any): string?
    function getupvalue(f: any, up: number): (string, any)?
    function setupvalue(f: any, up: number, value: any): string?
    function getuservalue(u: any): any
    function setuservalue(u: any, value: any): any
    function getmetatable(value: any): any
    function setmetatable(value: any, table: any): any
    function getregistry(): any
    function gethook(thread?: thread): any
    function sethook(thread?: thread, hook: any, mask: string, count?: number): void
}

// IO namespace
declare namespace io {
    type File = {
        close(): boolean
        flush(): boolean
        lines(): () => string?
        read(...: string): ...: any
        seek(whence?: string, offset?: number): number
        setvbuf(mode: string, size?: number): boolean
        write(...: string | number): File
    }
    
    function close(file?: File): boolean
    function flush(): boolean
    function input(file?: string | File): File
    function lines(filename?: string, ...: any): () => any?
    function open(filename: string, mode?: string): File?
    function output(file?: string | File): File
    function popen(prog: string, mode?: string): File?
    function read(...: string): ...: any
    function tmpfile(): File
    function type(obj: any): string?
    function write(...: string | number): File
    
    declare stdin: File
    declare stdout: File
    declare stderr: File
}

// Math namespace
declare namespace math {
    declare abs: (x: number) => number
    declare acos: (x: number) => number
    declare asin: (x: number) => number
    declare atan: (y: number, x?: number) => number
    declare ceil: (x: number) => number
    declare cos: (x: number) => number
    declare deg: (x: number) => number
    declare exp: (x: number) => number
    declare floor: (x: number) => number
    declare fmod: (x: number, y: number) => number
    declare log: (x: number, base?: number) => number
    declare max: (...: number) => number
    declare maxinteger: number
    declare min: (...: number) => number
    declare mininteger: number
    declare modf: (x: number) => number, number
    declare pi: number
    declare rad: (x: number) => number
    declare random: (m?: number, n?: number) => number
    declare randomseed: (x: number) => void
    declare sin: (x: number) => number
    declare sqrt: (x: number) => number
    declare tan: (x: number) => number
    declare tointeger: (x: any) => number?
    declare type: (x: any) => string
    declare ult: (m: number, n: number) => boolean
}

// OS namespace
declare namespace os {
    function clock(): number
    function date(format?: string, time?: number): string | any
    function difftime(t2: number, t1: number): number
    function execute(command?: string): boolean | number
    function exit(code?: boolean | number, close?: boolean): never
    function getenv(varname: string): string?
    function remove(filename: string): boolean, string?
    function rename(oldname: string, newname: string): boolean, string?
    function setlocale(locale: string, category?: string): string?
    function time(table?: any): number
    function tmpname(): string
}

// Package namespace
declare namespace package {
    declare config: string
    declare cpath: string
    declare loaded: { [string]: any }
    declare loadlib: (libname: string, funcname: string) => any
    declare path: string
    declare preload: { [string]: () => any }
    declare searchers: { [number]: (string) => any }
    declare searchpath: (name: string, path: string, sep?: string, rep?: string) => string?
}

// String namespace
declare namespace string {
    declare byte: (s: string, i?: number, j?: number) => ...: number
    declare char: (...: number) => string
    declare dump: (f: () => any, strip?: boolean) => string
    declare find: (s: string, pattern: string, init?: number, plain?: boolean) => number?, number?, ...: any
    declare format: (formatstring: string, ...: any) => string
    declare gmatch: (s: string, pattern: string) => () => ...: any
    declare gsub: (s: string, pattern: string, repl: string | (...: any) => string, n?: number) => string, number
    declare len: (s: string) => number
    declare lower: (s: string) => string
    declare match: (s: string, pattern: string, init?: number) => ...: any
    declare pack: (fmt: string, ...: any) => string
    declare packsize: (fmt: string) => number
    declare rep: (s: string, n: number, sep?: string) => string
    declare reverse: (s: string) => string
    declare sub: (s: string, i: number, j?: number) => string
    declare unpack: (fmt: string, s: string, pos?: number) => ...: any
    declare upper: (s: string) => string
}

// Table namespace
declare namespace table {
    declare concat: (list: { [number]: string }, sep?: string, i?: number, j?: number) => string
    declare insert: (list: { [number]: any }, pos?: number, value?: any) => void
    declare move: (a1: { [number]: any }, f: number, e: number, t: number, a2?: { [number]: any }) => { [number]: any }
    declare pack: (...: any) => { [number]: any, n: number }
    declare remove: (list: { [number]: any }, pos?: number) => any
    declare sort: (list: { [number]: any }, comp?: (a: any, b: any) => boolean) => void
    declare unpack: (list: { [number]: any }, i?: number, j?: number) => ...: any
}

// UTF8 namespace
declare namespace utf8 {
    declare char: (...: number) => string
    declare charpattern: string
    declare codes: (s: string) => () => number, number
    declare codepoint: (s: string, i?: number, j?: number) => ...: number
    declare len: (s: string, i?: number, j?: number) => number?
    declare offset: (s: string, n?: number, i?: number) => number?
}
