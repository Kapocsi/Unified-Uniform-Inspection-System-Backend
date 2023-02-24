/// Responsible for storing data about authenticated users
pub mod database {
    use serde::Deserialize;
    use serde::Serialize;
    use serde_json;
    use std::fs;

    use chrono;
    use crypto::bcrypt;
    use rand::random;

    const USERNAMES_PATH: &str = "./database/auth_users/usernames.csv";

    pub enum TokenResponse {
        Valid,
        Invalid,
        Expired,
    }
    #[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
    pub struct Token {
        uuid: String,
        token: [u8; 32],
        expirery: i64,
    }

    #[derive(Serialize, Deserialize, Clone)]
    pub struct User {
        uuid: String,

        username: String,
        password_hash: [u8; 24],
        salt: [u8; 16],
        pub tokens: Vec<Token>,
    }

    pub enum UserError {
        UsernameDuplicate,
        //UserNotFound,
        //CredentialsIncorrect,
    }

    impl User {
        pub fn new(username: String, password: String) -> Result<Self, UserError> {
            use uuid::Uuid;
            // check if username_taken
            let usernames = fs::read_to_string(USERNAMES_PATH.to_string())
                .expect("We should always have this file available");

            let username_taken: bool = &usernames
                .split(",")
                .filter(|f| f.clone().clone() == username)
                .count()
                >= &1;
            match username_taken {
                true => Err(UserError::UsernameDuplicate),
                false => {
                    let salt: [u8; 16] = random();

                    fs::write(
                        USERNAMES_PATH.to_string(),
                        format!("{},{}", usernames, username),
                    )
                    .expect("we should be able to write here at all times");

                    Ok(User {
                        salt: salt.clone(),
                        uuid: Uuid::new_v4().to_string(),
                        username,
                        password_hash: {
                            let mut hash: [u8; 24] = [0; 24];
                            bcrypt::bcrypt(10, &salt, password.as_bytes(), &mut hash);
                            hash
                        },
                        tokens: vec![],
                    })
                }
            }
        }

        pub fn get_user(username: String, password: String) -> Option<Self> {
            let users: Vec<User> = serde_json::de::from_str(
                &fs::read_to_string("./database/auth_users/users.json")
                    .expect("We should always get file here"),
            )
            .expect("we should always have a good json");

            let mut user = None::<User>;
            for u in users {
                if u.username == username && {
                    let mut output: [u8; 24] = [0; 24];
                    bcrypt::bcrypt(10, &u.salt, password.as_bytes(), &mut output);
                    output
                } == u.password_hash
                {
                    user = Some(u);
                }
            }
            return user;
        }

        pub fn authenticate_user(username: String, password: String) -> bool {
            let users: Vec<User> = serde_json::de::from_str(
                &fs::read_to_string("./database/auth_users/users.json")
                    .expect("We should always get file here"),
            )
            .expect("we should always have a good json");

            users
                .iter()
                .filter(|f| {
                    (f.username == username)
                        && (f.password_hash == {
                            let mut hash: [u8; 24] = [0; 24];
                            bcrypt::bcrypt(10, &f.salt, password.as_bytes(), &mut hash);
                            hash
                        })
                })
                .count()
                == 1
        }

        pub fn accosiate_token(&mut self) {
            self.tokens.push(Token::new(self.uuid.clone()))
        }

        pub fn push_to_disk(self) {
            let mut users: Vec<Self> = serde_json::from_str(
                &fs::read_to_string("./database/auth_users/users.json")
                    .expect("Could not read users.json")
                    .as_str(),
            )
            .unwrap();

            match users.iter().position(|f| f.uuid == self.uuid) {
                None => users.push(self),
                Some(t) => users[t] = self,
            };

            fs::write(
                "./database/auth_users/users.json",
                serde_json::to_string(&users).expect("failed to Serialize"),
            )
            .expect("Failed to write to disk");
        }
    }

    impl Token {
        pub fn new(uuid: String) -> Self {
            Token {
                uuid,
                token: random(),
                // Set expiry to on day in the future
                expirery: chrono::Utc::now().timestamp() + 86_400,
            }
        }

        pub fn check_token_validy(&self) -> TokenResponse {
            let users: Vec<User> = serde_json::de::from_str(
                &fs::read_to_string("./database/auth_users/users.json")
                    .expect("We should always get file here"),
            )
            .expect("we should always have a good json");

            let refrenced_user = users
                .iter()
                .filter(|f| f.uuid == self.uuid)
                .collect::<Vec<&User>>();

            match refrenced_user.first() {
                None => TokenResponse::Invalid,
                Some(t) => {
                    // check if token is present
                    let is_present = t.tokens.contains(self);
                    // check if not expired
                    let is_not_expired = self.expirery >= chrono::Utc::now().timestamp();

                    match (is_present, is_not_expired) {
                        (true, true) => TokenResponse::Valid,
                        // token expired
                        (true, false) => TokenResponse::Expired,

                        _ => TokenResponse::Invalid,
                    }
                }
            }
        }
    }
}
