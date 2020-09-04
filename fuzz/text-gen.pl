#!/usr/bin/env perl
use strict;
use warnings;
use String::Random;
use Crypt::Random qw( makerandom_itv ); 
use String::Random qw(random_regex random_string);
use IO::Socket;
use IO::Select;
use List::Util qw(any shuffle);
use Time::HiRes qw( usleep );

my @nicknames = qw{
AimHere
albertito amateurhour Andrio Anerag Aniem Announcy aoei aosdict   
APic ArjanS arkoldthos askhl baloona beardy Belinnon bezaban bhaak
bildramer Bleem bocaneri bouquet bowerman C5OK5Y CarlGel cfricke  
cheekio ChrisE Chromaryu ConductCat crumbking crummel cyphase Cypir
ddevault deepy def_jam DiffieHellman dimestop Dirm dom96 Dracunos 
dtype e2 eb0t_ eggandhull eki el eldritch elenmirie__ empty_string
enkrypt evh explodes f1reflyylmao f6k fadein farfar Fear__ FIQ    
fizzie francisv` friki Frogging101 fstd Gaelan galaxy_knuckles    
ghormoon glamas GoldenBear_ greeter gregdek grumble Guest8377     
Gustavo6046 GyroW Haitch Hansformer Heavylobster heinrich5991     
heredoc hierbat higuita hisacro HiSPeed Hydroxide iamruinous icfx 
igemnace illusion inire introsp3ctive irina|log itsblah j6p jilles
Jira johnla johnsu01 jonadab Joonaa jumpula k-man K2 kawzeg khoR  
Krakhan Learath2 lonjil lorimer madalu MaryHadalittle matthewbauer
mauregato mekkis Menchers michagogo misha miton moon-child moony  
Moult mplsCorwin Muad mud mursu myfreeweb nabru namad7 NAOrsa     
Neko-chan_ nengel neunon NeuroWinter nicole Nidan NightMonkey     
nkuttler normen noxd Nyoxi oldlaptop panda_ PavelB paxed pfn Phoul
Pigeonburger Pio PMunch poollovernathan port443 prg318 programmerq
PyroLagus raisse rast-- RaTTuS|BIG rebatela Renter_ rodgort Rodney
rsarson runcible ruskie s_edrik Sabotender Schroeder Sec SegFault 
serhei sgun sigma_g skelly skyenet slondr Smiley solexx specing   
spiffytech spiffytech192 starly stenno2 svt Tanoc_ TarrynPellimar 
tarzeau TAS_2012v theokperson theorbtwo thesquib Thisisbilly      
tijara towo_ trn tungtn_ tux3 unsound uovobw VaderFLAG Vejeta vent
winny Wooble Xlbrag yeled yidhra zoid zorkian zyith {Demo}2
};


my $handle;
my $path_to_file = "fuzz/50-chans.txt";
unless (open $handle, "<:encoding(utf8)", $path_to_file) {
   die "Could not open file '$path_to_file'\n";
}
chomp(my @channels = <$handle>);
unless (close $handle) {
   # what does it mean if close yields an error and you are just reading?
   die "Don't care error while closing '$path_to_file'\n";
}

my @commands = ("NICK", "USER", "JOIN", "PART", "PRIVMSG", "NOTICE");
my @targets = (@channels, @nicknames);
my $nick = rand_from(@nicknames);
my $user = rand_from(@nicknames);
my $string_gen = String::Random->new;
my $realname = $string_gen->randregex('[A-Z]{2}[a-z]{2}.[a-z]{2}\d');
my $log_path="fuzz/logs/${nick}";
my $sock = IO::Socket::INET->new(
    PeerAddr => '127.0.1.1',
    PeerPort => 6667,
    Proto => 'tcp',
    Blocking => 0
);
my $flags = 0;
if ($sock->connected()) {
    print "connection established\n";
    my $buf = $sock->getsockopt(SOL_SOCKET, SO_RCVBUF);
    print "buffer is $buf bytes\n";
}
my $s = IO::Select->new();
$s->add($sock);
my $sent = $sock->send("NICK ${nick}\r\n", $flags);
$sent = $sock->send("USER ${user} . . :${realname}\r\n", $flags);

unless (open $handle, ">:encoding(utf8)", $log_path) {
   print STDERR "Could not open file '$log_path': $!\n";
   # we return 'undefined', we could also 'die' or 'croak'
   return undef;
}
my ($buffer, $string, $n_targets, $cmd, $read, $victims, @ready);
my $seed = my_randint(20);
while (1) {
    usleep(my_randint(1000000*$seed));
    @ready = $s->can_read(0);
    if (any { $_ eq $sock } @ready) {
        $read = $sock->recv($buffer, 4096);
        print $handle $buffer;
        if ($buffer =~ /^:.* (PRVIMSG|JOIN|PART)/) { print $buffer }
    }


    $cmd = rand_from(@commands);
    $n_targets = my_randint(15);
    $string = $cmd;
    if (my_randint(10) < 5) {
        $n_targets = 1; ## bias to single targets a bit
    }
    $victims = get_targets($n_targets, @targets);
    $string .= " $victims";
    if (my_randint(10) > 6) {
        # additional data
        $string .= " :" . $string_gen->randregex('[A-Z]{8}[a-z]{9}.[a-z]{4}\d');
    }

    # add a random prefix
    if (my_randint(20) eq 18) {
        $string = ":" . gen_prefix() . " " . $string;
    }

    if (my_randint(40) < 3) {
        $string = shuffle($string);
    }
#    apparently injecting byte data into the stream crashes perl, not rusty-ircd...
#    Well, not sure - what was that error? could not determine peer address...
#    if (my_randint(20) eq 1) {
#        # inject random bytecode into $string
#        my $length = my_randint(length($string));
#        my $count = 0;
#        while ($count < $length) {
#            my $index = my_randint(length($string));
#            my $sub_length = my_randint(length($string)/5);
#            my $bytes = join "", map { pack("C*", my_randint(255)) } (0..$sub_length);
#            my $sub = substr $string, $index, $sub_length, $bytes;
#            $count += $sub_length;
#        }
#        print "injected some bytes";
#    }

    if (my_randint(300) eq 1) {
        exit 0
    }
    $string .= "\r\n";
    $sent = $sock->send($string, $flags);
    if (length($string) eq $sent) {
        print $handle $string;
        if ($string =~ /^:.* /) { print $string }
    } 
}

sub get_targets {
    my $number = shift;
    my @list = @_;

    my @selected;
    while (@selected < $number) {
        push @selected, rand_from(@list);
    }
    return join ",", @selected;
}

sub my_randint {
    my $target = shift;
    my $r = int( makerandom_itv ( Lower => 0, Upper => int($target) ));
    my $result = $r;
    return $result;
}

sub rand_from {
    my @array = @_;
    my $len = @array;
    my $index = my_randint($len);
    return $array[$index];
}

sub gen_prefix {
    my $string_gen = String::Random->new;
    my $regex = '[A-Za-z]{' . my_randint(8) . '}';
    my $nick = $string_gen->randregex($regex);
    $regex = '[A-Za-z]{' . my_randint(8) . '}';
    my $user = $string_gen->randregex($regex);
    $regex = '[A-Za-z]{' . my_randint(16) . '}';
    my $host = $string_gen->randregex($regex);
    return ${nick} . '!' . ${user} . '@' . ${host};
}


#$path_to_file = "fuzz/shorter-chanlist.txt";
#unless (open $handle, ">:encoding(utf8)", $path_to_file) {
#   die "Could not open file '$path_to_file'\n";
#}
#foreach my $i (1..1000) {
#    print $handle rand_from(@channels) . "\n";
#}
#unless (close $handle) {
#   # what does it mean if close yields an error and you are just reading?
#   die "Don't care error while closing '$path_to_file'\n";
#}
#exit 0;
