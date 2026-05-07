use activitypub_federation::http_signatures::Keypair;
use anyhow::Result;

// Use a hardcoded keypair for testing since generating one for each test is slow.
#[cfg(test)]
const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDM694SJDJe8ebG\n\
l6MesbIzn/LZ5FO4ZeuM2grerkPxZ2FrWVM16Az5NFH4cySD7uTnzdQEjSkCx/OC\n\
4fGlvcqVmzqtqcwI3wfjMI7ihclNcZG+ZYI9ucrbpWglPJeUr1ORkYswNOuGrw8P\n\
imyrHZfSDxPkChDEbX3QpDi/Au9uxhanLuAxabQ41SaANixUemvKcPGoF9NQzjsq\n\
M8fcaTmy/H1wdeW6Fyf8oz0U5gTL1Smn0xyJMDB/dvUGdbT+S70DQhfND4suCtKQ\n\
KsfpB64kNmmnZ1xw/ehWSEx5Jyj2tP3sQm1dUFvGVDwsPe3bPGwz0TbgzVfovODK\n\
uGYkLDR3AgMBAAECggEADEkDukCzSF/mPveuTtPGZDPtokR/BGoP0hTsW+dEqX9S\n\
JtZnF68+v930IBn/EW3MCV2cnV09HS6RmcIj85TKRWfV/71TPyDn3yX1Gv18UQlC\n\
/JAnW738vGhRMxJL4B0WvH5mQtCZYiyykXLyCtwuUuiWf7BeyvfpeA2wXTs20YQx\n\
NoQr53PxQnD7LBr9OPuxQmef5E0o1aOlw9bMXxOVMEL4zLj/xhvJ0gv/Rn7C8dxB\n\
cuvKcuiVjBpLYa1TT38aGp8oOStl9GCmhCOij9FWLVBwzbQGGAmksLX6KkRw2h6Q\n\
EBSVjRUu6I9t/lorKkFoerPSCMICi2Yj/sfGFib02QKBgQDntdYzzdrpctfTtMdJ\n\
ET20M4dRoVqaj0U83LdKSEMp0x4JAFZHzn6qnxeZ8dS6aXevHxpiPdcC4WdZ6rSW\n\
a28La08E3CCHThZ/vNDvP+ShERKCLSqa+bUQ9klDfn8hGTD6w0d0Mhx8v4wy1IVA\n\
L8UWQY/G2E9D79UCOFgtlGNiswKBgQDiZx/UWrTcu0nG9o5m8h9yMQPt0/qXHaKS\n\
0+7C6dUezjN66Iu+joP4U5agsbFZ+zWVK+xOvZDKy52dlhKAMJREBK3B7igVi/5M\n\
pkC3JAdcIeP6XQ72wo608wLSfgmgnvA5uCb6XP/gegVxaW7z4YI78RAyStGndcQj\n\
UStSkF45LQKBgQCR2He6ZdFr7biB7iEeEbcYDPMY44onDRUUqQzJudBkrBkUq1yj\n\
mAtMlBUD9h7jMu19kgNGYQxMKNqn0z7WC0t7EZFMSs5CvFkXEB8m6L2c0CUpQQq3\n\
P4PD6HHXBPE6QSP+QxpfvgcGUn8Jo5E4BJl2V9AK5i6GYZhe7F48WlFwVwKBgQC4\n\
Wag9Ta6/nXExpUnG4Zhhby/31AfUTLk4PYHJDIYRE24vwnMnsvwalFWue4Ih9r9m\n\
u+ErLIhd2PZ6ftyJrQTNbdHee4IAKYHj/+vqNFgZ2S69ilDI9RsmlEnUA/Tq6QBK\n\
v3xdmKRxsGoGMwe5ZgKZtGyvxuR4KxiOeWWBUTSn8QKBgC7Y8QacsOEFT9KQ6aT7\n\
HQIY5+Pf5CkiBV873MDZl3cd7F9z5WAcCv9EatoCeK1TWjkrHL7NRBz0djMVHSCG\n\
YC8FTA5UGgdd1JwId59Tnj0tI4ynqKy8QS6x5U8SNGx1PhGzcfR8EDoocMP8Y3Wg\n\
NibMISpyRl/tjPoQvtYPcvzb\n\
-----END PRIVATE KEY-----";

#[cfg(test)]
const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\n\
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAzOveEiQyXvHmxpejHrGy\n\
M5/y2eRTuGXrjNoK3q5D8Wdha1lTNegM+TRR+HMkg+7k583UBI0pAsfzguHxpb3K\n\
lZs6ranMCN8H4zCO4oXJTXGRvmWCPbnK26VoJTyXlK9TkZGLMDTrhq8PD4psqx2X\n\
0g8T5AoQxG190KQ4vwLvbsYWpy7gMWm0ONUmgDYsVHprynDxqBfTUM47KjPH3Gk5\n\
svx9cHXluhcn/KM9FOYEy9Upp9MciTAwf3b1BnW0/ku9A0IXzQ+LLgrSkCrH6Qeu\n\
JDZpp2dccP3oVkhMeSco9rT97EJtXVBbxlQ8LD3t2zxsM9E24M1X6LzgyrhmJCw0\n\
dwIDAQAB\n\
-----END PUBLIC KEY-----";

#[cfg(test)]
pub fn generate_keypair() -> Result<Keypair> {
    Ok(Keypair {
        private_key: PRIVATE_KEY.to_owned(),
        public_key: PUBLIC_KEY.to_owned(),
    })
}

#[cfg(not(test))]
pub fn generate_keypair() -> Result<Keypair> {
    use activitypub_federation::http_signatures::generate_actor_keypair;
    Ok(generate_actor_keypair()?)
}
