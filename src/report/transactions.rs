use crate::report::encoding::{Field, write_register_row};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use std::io::{self, Write};

/// Common fields shared across all transaction records
#[derive(Debug)]
pub struct TransactionBase {
    /// Data da operação no formato DDMMAAAA
    pub operation_date: NaiveDate,
    /// Valor das taxas em reais, cobradas na operação
    pub operation_fees: Option<Decimal>,
    /// Símbolo do criptoativo (ex: BTC, ETH)
    pub crypto_symbol: String,
    /// Quantidade de criptoativos
    pub crypto_amount: Decimal,
}

/// Common fields for exchange information
#[derive(Debug)]
pub struct ExchangeInfo {
    /// Nome da exchange domiciliada no exterior
    pub name: String,
    /// Endereço da internet da exchange domiciliada no exterior
    pub url: String,
    /// Código de identificação do país de domicílio fiscal da exchange
    pub country: String,
}

impl ExchangeInfo {
    /// Returns the exchange fields in the correct order
    fn fields(&self) -> Vec<Field<'_>> {
        vec![
            Field::AlphaNumber { value: &self.name },
            Field::AlphaNumber { value: &self.url },
            Field::AlphaNumber {
                value: &self.country,
            },
        ]
    }
}

impl TransactionBase {
    /// Returns the common fields for this transaction base in the correct order
    fn common_fields<'a>(&'a self, record_code: &'a str) -> Vec<Field<'a>> {
        vec![
            Field::Date(self.operation_date),
            Field::AlphaNumber { value: record_code },
            self.operation_fees
                .as_ref()
                .map_or(Field::Empty, |fees| Field::DecimalNumber {
                    value: fees,
                    precision: 2,
                }),
            Field::AlphaNumber {
                value: &self.crypto_symbol,
            },
            Field::DecimalNumber {
                value: &self.crypto_amount,
                precision: 10,
            },
        ]
    }
}

/// Registro 0110: Registra as operações de compra
#[derive(Debug)]
pub struct PurchaseTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Valor da operação em reais, excluídas as taxas
    pub operation_value: Decimal,
    /// Informações da exchange do comprador
    pub buyer_exchange: ExchangeInfo,
}

/// Registro 0120: Registra as operações de venda
#[derive(Debug)]
pub struct SaleTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Valor da operação em reais, excluídas as taxas
    pub operation_value: Decimal,
    /// Informações da exchange do vendedor
    pub seller_exchange: ExchangeInfo,
}

/// Registro 0210: Registra as operações de permuta
#[derive(Debug)]
pub struct SwapTransaction {
    /// Data da operação no formato DDMMAAAA
    pub operation_date: NaiveDate,
    /// Valor das taxas em reais, cobradas na operação
    pub operation_fees: Option<Decimal>,
    /// Símbolo do criptoativo recebido
    pub received_crypto_symbol: String,
    /// Quantidade de criptoativos recebidos
    pub received_crypto_amount: Decimal,
    /// Símbolo do criptoativo entregue
    pub given_crypto_symbol: String,
    /// Quantidade de criptoativos entregues
    pub given_crypto_amount: Decimal,
    /// Informações da exchange
    pub exchange: ExchangeInfo,
}

/// Registro 0410: Registra as operações de transferência de criptoativo para Exchange
#[derive(Debug)]
pub struct TransferToExchangeTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Código alfanumérico que representa a wallet do cliente na Exchange
    pub origin_wallet: Option<String>,
    /// Nome da exchange estrangeira de origem do criptoativo
    pub origin_exchange_name: Option<String>,
}

/// Registro 0510: Registra as operações de retirada de criptoativo da Exchange
#[derive(Debug)]
pub struct WithdrawalFromExchangeTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Informações da exchange de origem
    pub origin_exchange: ExchangeInfo,
}

/// Registro 0710: Registra as operações de dação de criptoativos em pagamento - Recebedor
#[derive(Debug)]
pub struct CryptoPaymentReceiverTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Informações da exchange do recebedor
    pub receiver_exchange: ExchangeInfo,
}

/// Registro 0720: Registra as operações de dação de criptoativos em pagamento - Pagador
#[derive(Debug)]
pub struct CryptoPaymentSenderTransaction {
    /// Base fields common to all transactions
    pub base: TransactionBase,
    /// Informações da exchange do pagador
    pub sender_exchange: ExchangeInfo,
}

/// Enum representing all possible transaction types
#[derive(Debug)]
pub enum Transaction {
    Purchase(PurchaseTransaction),
    Sale(SaleTransaction),
    Swap(SwapTransaction),
    TransferToExchange(TransferToExchangeTransaction),
    WithdrawalFromExchange(WithdrawalFromExchangeTransaction),
    CryptoPaymentReceiver(CryptoPaymentReceiverTransaction),
    CryptoPaymentSender(CryptoPaymentSenderTransaction),
}

impl Transaction {
    /// Returns the record type code for this transaction
    pub fn record_type(&self) -> (&'static str, &'static str) {
        match self {
            Transaction::Purchase(_) => ("0110", "I"),
            Transaction::Sale(_) => ("0120", "I"),
            Transaction::Swap(_) => ("0210", "II"),
            Transaction::TransferToExchange(_) => ("0410", "IV"),
            Transaction::WithdrawalFromExchange(_) => ("0510", "V"),
            Transaction::CryptoPaymentReceiver(_) => ("0710", "VII"),
            Transaction::CryptoPaymentSender(_) => ("0720", "VII"),
        }
    }

    /// Writes the transaction to the given writer in the report format
    pub fn write_transaction<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let (record_type, record_code) = self.record_type();
        let fields = match self {
            Transaction::Purchase(t) => {
                let mut fields = vec![
                    Field::AlphaNumber { value: record_type },
                    Field::Date(t.base.operation_date),
                    Field::AlphaNumber { value: record_code },
                    Field::DecimalNumber {
                        value: &t.operation_value,
                        precision: 2,
                    },
                    t.base.operation_fees.as_ref().map_or(Field::Empty, |fees| {
                        Field::DecimalNumber {
                            value: fees,
                            precision: 2,
                        }
                    }),
                    Field::AlphaNumber {
                        value: &t.base.crypto_symbol,
                    },
                    Field::DecimalNumber {
                        value: &t.base.crypto_amount,
                        precision: 10,
                    },
                ];
                fields.extend(t.buyer_exchange.fields());
                fields
            }
            Transaction::Sale(t) => {
                let mut fields = vec![
                    Field::AlphaNumber { value: record_type },
                    Field::Date(t.base.operation_date),
                    Field::AlphaNumber { value: record_code },
                    Field::DecimalNumber {
                        value: &t.operation_value,
                        precision: 2,
                    },
                    t.base.operation_fees.as_ref().map_or(Field::Empty, |fees| {
                        Field::DecimalNumber {
                            value: fees,
                            precision: 2,
                        }
                    }),
                    Field::AlphaNumber {
                        value: &t.base.crypto_symbol,
                    },
                    Field::DecimalNumber {
                        value: &t.base.crypto_amount,
                        precision: 12,
                    },
                ];
                fields.extend(t.seller_exchange.fields());
                fields
            }
            Transaction::Swap(t) => {
                let mut fields = vec![
                    Field::AlphaNumber { value: record_type },
                    Field::Date(t.operation_date),
                    Field::AlphaNumber { value: record_code },
                    t.operation_fees
                        .as_ref()
                        .map_or(Field::Empty, |fees| Field::DecimalNumber {
                            value: fees,
                            precision: 2,
                        }),
                    Field::AlphaNumber {
                        value: &t.received_crypto_symbol,
                    },
                    Field::DecimalNumber {
                        value: &t.received_crypto_amount,
                        precision: 10,
                    },
                    Field::AlphaNumber {
                        value: &t.given_crypto_symbol,
                    },
                    Field::DecimalNumber {
                        value: &t.given_crypto_amount,
                        precision: 10,
                    },
                ];
                fields.extend(t.exchange.fields());
                fields
            }
            Transaction::TransferToExchange(t) => {
                let mut fields = vec![Field::AlphaNumber { value: record_type }];
                fields.extend(t.base.common_fields(record_code));
                fields.extend(vec![
                    t.origin_wallet
                        .as_ref()
                        .map_or(Field::Empty, |w| Field::AlphaNumber { value: w }),
                    t.origin_exchange_name
                        .as_ref()
                        .map_or(Field::Empty, |n| Field::AlphaNumber { value: n }),
                ]);
                fields
            }
            Transaction::WithdrawalFromExchange(t) => {
                let mut fields = vec![Field::AlphaNumber { value: record_type }];
                fields.extend(t.base.common_fields(record_code));
                fields.extend(t.origin_exchange.fields());
                fields
            }
            Transaction::CryptoPaymentReceiver(t) => {
                let mut fields = vec![Field::AlphaNumber { value: record_type }];
                fields.extend(t.base.common_fields(record_code));
                fields.extend(t.receiver_exchange.fields());
                fields
            }
            Transaction::CryptoPaymentSender(t) => {
                let mut fields = vec![Field::AlphaNumber { value: record_type }];
                fields.extend(t.base.common_fields(record_code));
                fields.extend(t.sender_exchange.fields());
                fields
            }
        };

        write_register_row(writer, &fields)
    }
}
