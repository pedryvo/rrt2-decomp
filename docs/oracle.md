# Oráculo: teste diferencial com o EXE original

`crates/rt2-oracle` carrega `re/bin/RT2_PLAT.EXE` (PE32, base `0x400000`) no emulador Unicorn e executa **uma função por vez**, para comparar bit a bit com a nossa implementação em Rust (docs/PLAN.md, seção 11).

## O que faz

- Mapeia cada seção no seu VA (BSS zerado), mais pilha, um heap de bump (`alloc`) e uma região de stubs.
- Cada slot da IAT aponta para um stub. Se o código emulado chamar um import sem hook, a emulação para com `ImportCalled { dll, name }` (`KERNEL32.dll!GetTickCount`). Hooks em Rust: `hook_import("nome", bytes_de_args, Box::new(|uc| ...))`; leia argumentos com `stack_arg`, o retorno vai em EAX.
- Chamadas: `call_cdecl`, `call_stdcall`, `call_thiscall(this, args)` (ECX), `call_fastcall(ecx, edx, args)`. Um endereço sentinela é empilhado como retorno; há limite de instruções (`set_instruction_limit`, padrão 10M).
- Resultados: valor de retorno, `eax()`, `edx()`, `edx_eax()` e `st0()` (x87, convertido para `f64`). Memória: `read_mem`/`write_mem`/`read_u32`/`write_u32`.
- `snapshot()` (feito no load) e `restore()` salvam/restauram as seções graváveis (`.data`).

## Como escrever um teste

1. Desmonte a função (`rizin -q -c 'pd 40 @ 0x4f0d70' re/bin/RT2_PLAT.EXE`) e descubra convenção e largura dos argumentos.
2. Em `crates/rt2-oracle/tests/`, carregue o EXE com `RT2_EXE` (padrão `re/bin/RT2_PLAT.EXE`) e saia com `eprintln!` se não existir, para o CI sem o binário passar.
3. Compare a saída do original com a função Rust para um conjunto exaustivo ou aleatório com semente fixa. Exemplo: `tests/fold.rs` (case fold do hash de nomes PK2, 256 valores).
4. Estruturas: aloque com `alloc_bytes`, passe o ponteiro, leia de volta com `read_mem`. Globais: escreva no endereço do `.data` e chame `restore()` entre casos.

## Limites

- Sem sistema operacional: nada de Win32, arquivos, tempo ou DirectX. Funções que tocam a IAT precisam de hook ou ficam fora do alcance.
- Sem TEB/FS, sem CRT inicializado (o ponto de entrada não roda): funções que dependem de estado do CRT (heap `malloc`, locale) podem falhar.
- Estado da FPU persiste entre chamadas e não é restaurado por `restore()`; se testar código x87, execute `fninit` (ou compare só `st0()` após reiniciar a pilha) e cuidado com a palavra de controle de precisão (o jogo pode esperar 24 ou 53 bits).
- O heap emulado é só de bump e nunca liberado.
- A lib Unicorn é do sistema (`libunicorn`, recurso `system-unicorn`, padrão). O CI compila a cópia embutida com `--no-default-features` (Linux) e não compila o crate no Windows.
- O laço do hash de nomes (`0x4f4e60`, `0x4f5070`) é inline em funções que usam I/O de arquivo; não há unidade chamável isolada, então só o case fold (`0x4f0d70`) é testado.
- A palavra de controle da FPU começa em 0 no Unicorn (precisão de 24 bits). Para código x87 que o original roda com o padrão do CRT MSVC, escreva `RegisterX86::FPCW = 0x27f` antes de chamar (`uc().reg_write`); ver `tests/sim_time_rng.rs` e `docs/spec/time.md`.
