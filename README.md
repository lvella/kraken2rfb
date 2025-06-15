# kraken2rfb

Esta é uma ferramenta em linha de comando para gerar o relatório mensal instituído
pela Instrução Normativa RFB nº 1.888, de 3 de maio de 2019, que obriga pessoas
físicas e jurídicas a enviarem um relatório com todas as movimentações de
criptoativos que realizaram em exchanges estrangeiras no mês anterior, se o valor
total movimentado tiver sido maior do que R$ 30.000,00.

Este programa utiliza as APIs da Kraken, CoinGecko e Banco Central do Brasil para
coletar todas as transações feitas na Kraken, converter os valores para R$ na
cotação do dia, e gerar o relatório conforme o [MANUAL DE ORIENTAÇÃO DO LEIAUTE DAS
INFORMAÇÕES RELATIVAS ÀS OPERAÇÕES REALIZADAS COM CRIPTOATIVOS](https://www.gov.br/receitafederal/pt-br/assuntos/orientacao-tributaria/declaracoes-e-demonstrativos/criptoativos/arquivos/ato-declaratorio-executivo-copes-ndeg-1-2023/manual-de-orientacao-do-leiaute-criptoativos-versao-1-2.pdf), versão 1.2.

Supondo que você só utilizou a Kraken no último mês, o relatório pode então ser
submetido pelo portal e-CAC, se você tiver uma versão suficientemente antiga do
Java e um certificado digital ICP-Brasil (login com gov.br ouro não é suficiente).

## Aviso

Esta é uma aplicação em estado alfa! Se resolver usar, confira sempre o relatório
gerado. Não há nenhuma mensagem de erro, só crasha em caso de erro. Nem todos os
tipos de transação foram testados, somente "compra" e "retirada".

Esta foi minha primeira tentativa de "vibe-coding". É seguro supor que várias linhas
de código nunca foram revisadas por olhos humanos. Foram algumas tardes de sábado
meio frustrantes, mas no final das contas, aparentemente funciona.
