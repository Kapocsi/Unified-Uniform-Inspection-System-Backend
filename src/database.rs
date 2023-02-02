#![warn(unused_imports, dead_code)]

pub mod data {
    use std::{fmt::format, fs};

    use crypto;
    use futures_util::future::ok;
    use rand;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Serialize, Deserialize, Debug)]
    pub enum Criteria {
        PassFail(CriteriaPassFail),
        Graded(CriteriaGraded),
        Comment(Option<String>),
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CriteriaPassFail {
        pub category_name: String,
        pub description: String,
        pub state: Option<bool>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct CriteriaGraded {
        /// This is where the text in the rubric should be scored, their index is taken to be the
        /// score
        pub category_name: String,
        pub descriptions: Vec<String>,
        pub state: Option<u8>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Inspection {
        pub name: String,
        pub criteria: Vec<Criteria>,
        pub date: Option<u64>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct User {
        // User Info
        pub username: Option<String>,
        pub uuid: String,
        pub inspections: Vec<Inspection>,
    }
    #[derive(Serialize, Deserialize, Debug)]
    pub struct AuthenticatedUser {
        user_name: String,
        uuid: String,

        salt: [u8; 16],
        password_hash: [u8; 24],

        pub auth_token: Option<AuthToken>,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    pub struct AuthToken {
        pub user_uuid: String,
        pub token: [u8; 32],
        pub expiry: i64,
    }

    impl AuthToken {
        fn new(uuid: String) -> AuthToken {
            AuthToken {
                user_uuid: uuid,
                token: rand::random(),
                // Set token to expire in one day
                expiry: chrono::Utc::now().timestamp() + 86_400,
            }
        }
        pub fn is_valid(&self) -> Result<bool, actix_web::error::Error> {
            let test_user: AuthenticatedUser = serde_json::from_str(
                fs::read_to_string(format!("database/auth_users/{}.json", self.user_uuid))
                    .map_err(|_| actix_web::error::ErrorNotFound("Could not find user"))?
                    .as_str(),
            )
            .expect("We should never have invalid json on the disk");

            println!("{:?}\n{:?}", self.clone(), test_user);

            Ok(Some(self) == test_user.auth_token.as_ref())
        }
    }

    impl User {
        pub fn new() -> User {
            User {
                username: None,
                uuid: Uuid::new_v4().to_string(),
                inspections: Vec::new(),
            }
        }
        pub fn push_to_data_base(&self) {
            fs::write(
                format!("database/users/{}.json", &self.uuid),
                serde_json::ser::to_string(&self).expect("Failed to serilize user"),
            )
            .expect("failed to write to disk")
        }
        pub fn push_inspection(&mut self, inspec: Inspection) {
            self.inspections.push(inspec);
        }
        pub fn read_from_database(uuid: String) -> Result<User, actix_web::error::Error> {
            let user: User = serde_json::de::from_str(
                fs::read_to_string(format!("database/users/{}.json", uuid))
                    .map_err(|_| actix_web::error::ErrorNotFound("Could not find specified user"))?
                    .as_str(),
            )
            .expect("This should always be valid json");
            Ok(user)
        }
    }

    impl AuthenticatedUser {
        pub fn new(user_name: String, password: String) -> AuthenticatedUser {
            // Generate Unique 32-char salt for user
            let salt = {
                let salt: [u8; 16] = rand::random();
                println!("{}", salt.len());
                salt
            };
            AuthenticatedUser {
                user_name,
                uuid: uuid::Uuid::new_v4().to_string(),
                password_hash: {
                    let mut password_hash: [u8; 24] = [0; 24];
                    crypto::bcrypt::bcrypt(12, &salt, &password.as_bytes(), &mut password_hash);
                    password_hash
                },
                salt,
                auth_token: None,
            }
        }
        pub fn push_to_data_base(self) {
            fs::write(
                format!("database/auth_users/{}.json", &self.uuid),
                serde_json::ser::to_string(&self).expect("Failed to serilize user"),
            )
            .expect("failed to write to disk")
        }

        pub fn read_from_data_base(
            username: String,
            password: String,
        ) -> Result<AuthenticatedUser, std::io::Error> {
            // Read the file names of all users;
            let user: Option<AuthenticatedUser> = fs::read_dir("database/auth_users/")?
                .map(|x| fs::read_to_string(x.unwrap().path()).unwrap())
                .map(|x| serde_json::de::from_str(x.as_str()).unwrap())
                .find(|x: &AuthenticatedUser| x.user_name == username);
            match user {
                Some(T) => {
                    if T.password_hash == {
                        let mut password_hash: [u8; 24] = [0; 24];
                        crypto::bcrypt::bcrypt(
                            12,
                            (&T.salt),
                            &password.as_bytes(),
                            &mut password_hash,
                        );
                        password_hash
                    } {
                        Ok(T)
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Incorrect Password",
                        ))
                    }
                }
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Not Found",
                )),
            }
        }

        pub fn attach_token(&mut self) {
            self.auth_token = Some(AuthToken::new(self.uuid.clone()));
        }
    }

    pub fn load_inspection_list() -> Result<Vec<Inspection>, std::io::Error> {
        let inspection_lists: Vec<Inspection> =
            serde_yaml::from_str(fs::read_to_string("./database/inspections.yaml")?.as_str())
                .expect("Invalid Yaml in Inspection List");

        Ok(inspection_lists)
    }
}
