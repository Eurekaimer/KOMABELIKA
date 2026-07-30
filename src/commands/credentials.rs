use anyhow::Result;

use crate::{
    cli::{CredentialArgs, CredentialProvider},
    credentials,
};

pub fn login(args: CredentialArgs) -> Result<()> {
    match args.provider {
        CredentialProvider::Deepseek => {
            let key = rpassword::prompt_password("DeepSeek API key: ")?;
            credentials::store_deepseek(key.trim())?;
            println!("DeepSeek API key saved in the system keyring.");
            Ok(())
        }
    }
}

pub fn logout(args: CredentialArgs) -> Result<()> {
    match args.provider {
        CredentialProvider::Deepseek => {
            if credentials::delete_deepseek()? {
                println!("DeepSeek API key removed from the system keyring.");
            } else {
                println!("No DeepSeek API key was stored in the system keyring.");
            }
            Ok(())
        }
    }
}
