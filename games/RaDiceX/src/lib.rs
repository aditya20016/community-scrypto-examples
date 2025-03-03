use scrypto::prelude::*;

#[derive(NonFungibleData)]
pub struct Ticket {
    #[mutable]
    level: i8,
    #[mutable]
    last_throw: String,
}
#[blueprint]
mod mod_radicex{
    struct Radicex {
        // vault to store radix, buy-in go here and prices are redeemed from here.
        radix_vault: Vault,
        
        // resourceaddress of the NFT ticket, used for NFT creation and various authorization
        my_non_fungible_ticket: ResourceAddress,

        // admin vault, this contains a badge for various inner dApp permission handling
        admin_vault: Vault,

        // keep track of the number of NFTs generated, this number will be used for the NFT-Id
        nrNFTsgenerated: u64,
    }

    impl Radicex {
        // Implement the functions and methods which will manage those resources and data
        // This is a function, and can be called directly on the blueprint once deployed
        pub fn instantiate() -> (ComponentAddress, Bucket) {

            // creating our admin badges
            // use one badge for internal admin stuff, and send one to instantiate wallet address.
            let mut my_admin_badge = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .metadata("name", "Admin Badge for RaDiceX")
                .mint_initial_supply(2);

            // put one admin badge and put in in the admin vault
            let local_admin_badge: Bucket = my_admin_badge.take(1);

            let admin_rule: AccessRule = rule!(require(my_admin_badge.resource_address()));

            // Create our Ticket NFT
            let my_non_fungible_ticket = ResourceBuilder::new_integer_non_fungible()
                .metadata("name", "Ticket for RaDiceX")
                .burnable(admin_rule.clone(), LOCKED)
                .mintable(rule!(require(my_admin_badge.resource_address())), LOCKED)
                .updateable_non_fungible_data(admin_rule.clone(), LOCKED)
                .restrict_withdraw(rule!(allow_all), LOCKED)
                .restrict_deposit(AccessRule::AllowAll, LOCKED)
                .create_with_no_initial_supply();

            // set the access rules for the Admin-only and internal functions.
            let access_rules = AccessRules::new()
                .method("admin_ticket", admin_rule.clone(), AccessRule::DenyAll)
                .method("withdrawal_all", admin_rule.clone(), rule!(deny_all))
                .method("roll_dice", rule!(deny_all), rule!(deny_all))
                .default(AccessRule::AllowAll, AccessRule::DenyAll);
                
            let mut component = Self {
                radix_vault: Vault::new(RADIX_TOKEN),
                my_non_fungible_ticket,
                admin_vault: Vault::with_bucket(local_admin_badge),
                nrNFTsgenerated: 0,
            }
          
            .instantiate();
            component.add_access_check(access_rules);
            let component = component.globalize();

            // Return the instantiated component and the admin badge that was just minted
            (component, my_admin_badge)
        }

        /*
            Die roll function, used internally
            This function is blocked for external call by access ruls
        */
        pub fn roll_dice(&mut self) -> i8 {
            let random = Runtime::generate_uuid();
            let dieval:i8 = ((random % 6) + 1) as i8;
            dieval
        }

        /*
            Die roll function, external available for comparison
        */
        pub fn roll_dice_old(&mut self) -> i8 {
            let random = Runtime::generate_uuid();
            let dieval:i8 = ((random % 6) + 1) as i8;
            dieval
        }
        /*
            Alterniative Die roll function, used internally, but should be external accessable
            The modulo (%) function is pratically a division, 
            this routine could be "cheaper" in network execution
        */
        pub fn roll_dice_alt(&mut self) -> i8 {
            loop{
                let mut random:u128 = Runtime::generate_uuid();
                while random > 0{
                    let myval = random & 0x7;
                    // if 0x6 or 0x7 throw away result and redo check on 3 new bits.
                    if myval < 0x6{ 
                        return (myval+1) as i8 ;
                    }
                    // get 3 new bits but shift 4 because 128/3 != integer
                    random = random >> 4; 
                }
            }
        }

        /*
            Deposit x coins in the main wallet so players can redeem their price.
        */
        pub fn deposit(&mut self, amount: Decimal, mut deposit: Bucket) -> Bucket {

            assert!(
                deposit.resource_address() == self.radix_vault.resource_address(),
                "The Buy-in can only be done with Radix tokens"
            );
            assert!(deposit.amount()>amount, "There are not enough tokens in your account");

            let xrd_deposit = deposit.take(amount);
            self.radix_vault.put(xrd_deposit);

            // return the bucket incase of a surplus of tokens
            deposit
        }

        /*
            Withdrawal all coin from the main wallet 
            Admin only function.
        */
        pub fn withdrawal_all(&mut self) -> Bucket {
            let xrd_withdrawal = self.radix_vault.take_all();
            xrd_withdrawal
        }

        /*
            Reinitialize a ticket that has NFT level field set to 0
            Using this method gives a 10% discount compared to buying new ticket.
        */
        pub fn reinit_ticket(&mut self, NFTTicket: Proof, mut buyin: Bucket) -> Bucket {

            assert!(
                buyin.resource_address() == self.radix_vault.resource_address(),
                "The Buy-in can only be done with Radix tokens"
            );
            assert!(!(buyin.amount()<dec!("0.9")), "Not enough XRD supplied");
            assert!(NFTTicket.amount()==dec!("1"), "Only one (1) ticket per call is supported");

            let validated_proof = NFTTicket.validate_proof(
                ProofValidationMode::ValidateResourceAddress(self.my_non_fungible_ticket)
            ).expect("invalid proof");

            let nft_id = validated_proof.non_fungible_local_id();
        
            let resource_manager: &mut ResourceManager = 
                borrow_resource_manager!(self.my_non_fungible_ticket);
        
            let mut ticket_data: Ticket = resource_manager.get_non_fungible_data(&nft_id);

            assert!(ticket_data.level == 0, "Level not 0, Ticket still playable");

            let amount: Decimal = dec!("0.9");

            let xrd_buy_in = buyin.take(amount);
            self.radix_vault.put(xrd_buy_in);

            ticket_data.level = 10;
            ticket_data.last_throw = "Just reinitialized the Ticket".to_string();
            
            self.admin_vault.authorize(|| resource_manager.update_non_fungible_data(
                &nft_id, 
                ticket_data
            ));

            buyin
        }

        /*
            Admin can retreive a free ticket 
            Admin only function
            function is also called by the buy_token function.
        */
        pub fn admin_ticket(&mut self) -> Bucket {
          
            let NFT_data = Ticket {
                level: 10,
                last_throw: "New Ticket, no play history".to_string(),
            };

            self.nrNFTsgenerated = self.nrNFTsgenerated.wrapping_add(1u64);

            let NFT_bucket = self.admin_vault.authorize(||{
                borrow_resource_manager!(self.my_non_fungible_ticket).mint_non_fungible(
                &NonFungibleLocalId::Integer(self.nrNFTsgenerated.into()),
                NFT_data
                )
            });

            NFT_bucket
        }

        /*
            Buy one RaDiceX ticket for 1 XRD, mint a NFT and send back
        */
        pub fn buy_ticket(&mut self, mut buyin: Bucket) -> (Bucket, Bucket) {

            // check if the buy-in bucket is XRD type, and hold enough coin
            assert!(
                buyin.resource_address() == self.radix_vault.resource_address(),
                "The Buy-in can only be done with Radix tokens"
            );
            assert!(!(buyin.amount()<dec!("1")), "Not enough XRD supplied");
            
            let amount: Decimal = dec!("1");
 
            let NFT_bucket = self.admin_ticket();

            let xrd_buy_in = buyin.take(amount);
            self.radix_vault.put(xrd_buy_in);
 
            (NFT_bucket, buyin)
        }

        /*
            redeem a price if the NFT level field is equal to 25
        */
        pub fn redeem_prize(&mut self, NFTTicket: Proof) -> Bucket {

            let redeem_amount: Decimal = dec!("5");
            assert!(&redeem_amount <= &self.radix_vault.amount(), 
                "Not enough funds in the vault to pay prize money");
            assert!(NFTTicket.amount()==dec!("1"), "Only one (1) ticket per call is supported");

            let validated_proof = NFTTicket.validate_proof(
                ProofValidationMode::ValidateResourceAddress(self.my_non_fungible_ticket)
            ).expect("invalid proof");

            let nft_id = validated_proof.non_fungible_local_id();
        
            let resource_manager: &mut ResourceManager = 
                borrow_resource_manager!(self.my_non_fungible_ticket);
        
            let mut ticket_data: Ticket = resource_manager.get_non_fungible_data(&nft_id);

            assert!(ticket_data.level == 25, "Level not 25, Ticket not redeemable");

            ticket_data.level = 0;
            ticket_data.last_throw = "Just redeemed a level 25 Ticket".to_string();
            
            self.admin_vault.authorize(|| resource_manager.update_non_fungible_data(
                &nft_id, 
                ticket_data
            ));

            let xrd_withdrawal =  self.radix_vault.take(redeem_amount);

            xrd_withdrawal
        }

        /*
            Play a round of RadiceX. Two dice are rolled
            the diff between the player value and house value is calculated and added to the NFT level field.
            If the player reaches level 25, this token can be redeemed for 5 XRD
            If the player reaches level 0, this ticket is no longer playable.
        */
        pub fn play_round(&mut self, NFTTicket: Proof) {

            assert!(NFTTicket.amount()==dec!("1"), "Only one (1) ticket per call is supported");

            let validated_proof = NFTTicket.validate_proof(
                ProofValidationMode::ValidateResourceAddress(self.my_non_fungible_ticket)
            ).expect("invalid proof");

            let nft_id = validated_proof.non_fungible_local_id();
        
            let resource_manager: &mut ResourceManager = 
                borrow_resource_manager!(self.my_non_fungible_ticket);
        
            let mut ticket_data: Ticket = resource_manager.get_non_fungible_data(&nft_id);

            assert!(ticket_data.level != 25, "Ticket Level = 25, Ticket not playable");
            assert!(ticket_data.level != 0, "Ticket Level = 0, Ticket not playable");

            let house_die = self.roll_dice();
            let player_die = self.roll_dice();
            let mut diff_of_dice = &player_die - &house_die;
            // code in case both house and player die-roll is equal
            // die[1]=-3;die[2]=-2;die[3]=-1;die[4]=0;die[5]=1;die[6]=2;
            if diff_of_dice == 0 {
                diff_of_dice = player_die.clone() - (4 as i8);
            }
            let mut newlevel = ticket_data.level + &diff_of_dice;
            if newlevel < 0{
                newlevel = 0;
            }
            if newlevel > 25{
                newlevel = 25
            }
            let throw_string: String = format!("House {}, Player {}, New Lvl {}({:+})", 
                                house_die, player_die, newlevel, diff_of_dice);
            ticket_data.level = newlevel;
            ticket_data.last_throw = throw_string;

            self.admin_vault.authorize(|| resource_manager.update_non_fungible_data(
                &nft_id, 
                ticket_data
            ));
        }
        /*
            Burning of a NFT ticket
        */
        pub fn burn_ticket(&mut self, NFTTicket: Bucket) {
            assert!(
                NFTTicket.resource_address() == self.my_non_fungible_ticket,
                "The supplied bucket does not contain the correct Ticket address"
            );
            assert!(!NFTTicket.is_empty(), "The supplied bucket is empty");
            assert!(NFTTicket.amount()==dec!("1"), "Only one (1) ticket per call is supported");

            let resource_manager: &mut ResourceManager = 
                borrow_resource_manager!(self.my_non_fungible_ticket);
    
            self.admin_vault.authorize(|| resource_manager.burn(NFTTicket));
        }

    }
}

