use crate::block::Block;
use ethabi::Token;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{BlockNumber, H256, U256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksCommitOperation {
    pub last_committed_block: Block,
    pub blocks: Vec<Block>,
}

pub fn stored_block_info(block: &Block) -> Token {
    Token::Tuple(vec![
        Token::Uint(U256::from(block.block_number)),
        Token::Uint(U256::from(block.number_of_processed_prior_ops())),
        Token::FixedBytes(
            block
                .get_onchain_operations_block_info()
                .1
                .as_bytes()
                .to_vec(),
        ),
        Token::Uint(U256::from(block.timestamp)),
        Token::FixedBytes(block.get_eth_encoded_root().as_bytes().to_vec()),
        Token::FixedBytes(block.block_commitment.as_bytes().to_vec()),
    ])
}

impl BlocksCommitOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        let stored_block_info = stored_block_info(&self.last_committed_block);
        let blocks_to_commit = self
            .blocks
            .iter()
            .map(|block| {
                let onchain_ops = block
                    .get_onchain_operations_block_info()
                    .0
                    .into_iter()
                    .map(|op| {
                        Token::Tuple(vec![
                            Token::Uint(U256::from(op.public_data_offset)),
                            Token::Bytes(op.eth_witness),
                        ])
                    })
                    .collect::<Vec<_>>();
                Token::Tuple(vec![
                    Token::Uint(U256::from(block.block_number)),
                    Token::Uint(U256::from(block.fee_account)),
                    Token::FixedBytes(block.get_eth_encoded_root().as_bytes().to_vec()),
                    Token::Bytes(block.get_eth_public_data()),
                    Token::Uint(U256::from(block.timestamp)),
                    Token::Array(onchain_ops),
                ])
            })
            .collect();

        vec![stored_block_info, Token::Array(blocks_to_commit)]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksProofOperation {
    pub commitments: Vec<(H256, BlockNumber)>,
}

impl BlocksProofOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        let recursive_input = Token::Array(vec![Token::Uint(U256::from(0)); 1]);
        let proof = Token::Array(vec![Token::Uint(U256::from(0)); 33]);
        let commitments = Token::Array(
            self.commitments
                .iter()
                .map(|(commitment, _)| {
                    Token::Uint(U256::from_big_endian(&commitment.to_fixed_bytes()))
                })
                .collect(),
        );
        let vk_indexes = Token::Array(vec![Token::Uint(U256::from(0)); self.commitments.len()]);
        let subproof_limbs = Token::FixedArray(vec![Token::Uint(U256::from(0)); 16]);
        vec![Token::Tuple(vec![
            recursive_input,
            proof,
            commitments,
            vk_indexes,
            subproof_limbs,
        ])]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExecuteOperationArg {
    pub block: Block,
    pub commitments: Vec<H256>,
    pub commitment_idx: usize,
}

impl BlockExecuteOperationArg {
    fn get_eth_tx_args(&self) -> Token {
        let stored_block = Token::Tuple(vec![
            Token::Uint(U256::from(self.block.block_number)),
            Token::Uint(U256::from(self.block.number_of_processed_prior_ops())),
            Token::FixedBytes(
                self.block
                    .get_onchain_operations_block_info()
                    .1
                    .as_bytes()
                    .to_vec(),
            ),
            Token::Uint(U256::from(self.block.timestamp)),
            Token::FixedBytes(self.block.get_eth_encoded_root().as_bytes().to_vec()),
            Token::FixedBytes(self.block.block_commitment.as_bytes().to_vec()),
        ]);

        let processable_ops_pubdata = Token::Array(
            self.block
                .processable_ops_pubdata()
                .into_iter()
                .map(|pubdata| Token::Bytes(pubdata))
                .collect(),
        );

        let commitments_in_slot = Token::Array(
            self.commitments
                .iter()
                .map(|comm| Token::FixedBytes(comm.as_bytes().to_vec()))
                .collect(),
        );

        let commitment_index = Token::Uint(U256::from(self.commitment_idx));

        Token::Tuple(vec![
            stored_block,
            processable_ops_pubdata,
            commitments_in_slot,
            commitment_index,
        ])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocksExecuteOperation {
    pub blocks: Vec<BlockExecuteOperationArg>,
}

impl BlocksExecuteOperation {
    pub fn get_eth_tx_args(&self) -> Vec<Token> {
        vec![Token::Array(
            self.blocks
                .iter()
                .map(|arg| arg.get_eth_tx_args())
                .collect(),
        )]
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AggregatedActionType {
    CommitBlocks,
    CreateProofBlocks,
    PublishProofBlocksOnchain,
    ExecuteBlocks,
}
impl std::string::ToString for AggregatedActionType {
    fn to_string(&self) -> String {
        match self {
            AggregatedActionType::CommitBlocks => "CommitBlocks".to_owned(),
            AggregatedActionType::CreateProofBlocks => "CreateProofBlocks".to_owned(),
            AggregatedActionType::PublishProofBlocksOnchain => {
                "PublishProofBlocksOnchain".to_owned()
            }
            AggregatedActionType::ExecuteBlocks => "ExecuteBlocks".to_owned(),
        }
    }
}

impl std::str::FromStr for AggregatedActionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CommitBlocks" => Ok(Self::CommitBlocks),
            "CreateProofBlocks" => Ok(Self::CreateProofBlocks),
            "PublishProofBlocksOnchain" => Ok(Self::PublishProofBlocksOnchain),
            "ExecuteBlocks" => Ok(Self::ExecuteBlocks),
            _ => Err("Incorrect aggregated action type".to_owned()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregatedOperation {
    CommitBlocks(BlocksCommitOperation),
    CreateProofBlocks(Vec<BlockNumber>),
    PublishProofBlocksOnchain(BlocksProofOperation),
    ExecuteBlocks(BlocksExecuteOperation),
}

impl AggregatedOperation {
    pub fn get_action_type(&self) -> AggregatedActionType {
        match self {
            AggregatedOperation::CommitBlocks(..) => AggregatedActionType::CommitBlocks,
            AggregatedOperation::CreateProofBlocks(..) => AggregatedActionType::CreateProofBlocks,
            AggregatedOperation::PublishProofBlocksOnchain(..) => {
                AggregatedActionType::PublishProofBlocksOnchain
            }
            AggregatedOperation::ExecuteBlocks(..) => AggregatedActionType::ExecuteBlocks,
        }
    }

    pub fn get_block_range(&self) -> (BlockNumber, BlockNumber) {
        match self {
            AggregatedOperation::CommitBlocks(BlocksCommitOperation { blocks, .. }) => (
                blocks.first().map(|b| b.block_number).unwrap_or_default(),
                blocks.last().map(|b| b.block_number).unwrap_or_default(),
            ),
            AggregatedOperation::CreateProofBlocks(blocks) => (
                blocks.first().cloned().unwrap_or_default(),
                blocks.last().cloned().unwrap_or_default(),
            ),
            AggregatedOperation::PublishProofBlocksOnchain(BlocksProofOperation {
                commitments,
            }) => (
                commitments.first().map(|c| c.1).unwrap_or_default(),
                commitments.last().map(|c| c.1).unwrap_or_default(),
            ),
            AggregatedOperation::ExecuteBlocks(BlocksExecuteOperation { blocks }) => (
                blocks
                    .first()
                    .map(|b| b.block.block_number)
                    .unwrap_or_default(),
                blocks
                    .last()
                    .map(|b| b.block.block_number)
                    .unwrap_or_default(),
            ),
        }
    }
}
